use std::{
    sync::mpsc::{Receiver, Sender},
    thread::{JoinHandle, scope},
    time::Duration,
};

use clashctl_core::Clash;
use crossterm::event::Event as CrossTermEvent;
use log::warn;
use rayon::prelude::*;

use crate::{
    interactive::Flags,
    ui::{
        Action, TuiOpt, TuiResult,
        event::{Event, UpdateEvent},
        utils::{Interval, Pulse},
    },
};

pub type Job = JoinHandle<TuiResult<()>>;

pub fn servo(tx: Sender<Event>, rx: Receiver<Action>, opt: TuiOpt, flags: Flags) -> TuiResult<()> {
    let clash = flags.connect_server_from_config()?;
    clash.get_version()?;

    scope(|r| -> TuiResult<()> {
        let tx_clone = tx.clone();
        let handle1 = r.spawn(|| input_job(tx_clone));

        let tx_clone = tx.clone();
        let handle2 = r.spawn(|| traffic_job(tx_clone, &clash));

        let tx_clone = tx.clone();
        let handle3 = r.spawn(|| log_job(tx_clone, &clash));

        let tx_clone = tx.clone();
        let handle4 = r.spawn(|| req_job(&opt, &flags, tx_clone, &clash));

        let handle5 = r.spawn(|| action_job(&opt, &flags, tx, rx, &clash));

        handle1.join().unwrap()?;
        handle2.join().unwrap()?;
        handle3.join().unwrap()?;
        handle4.join().unwrap()?;
        handle5.join().unwrap()?;

        Ok(())
    })
}

fn input_job(tx: Sender<Event>) -> TuiResult<()> {
    loop {
        match crossterm::event::read() {
            Ok(CrossTermEvent::Key(event)) => tx.send(Event::from(event))?,
            Err(_) => {
                tx.send(Event::Quit)?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn req_job(_opt: &TuiOpt, _flags: &Flags, tx: Sender<Event>, clash: &Clash) -> TuiResult<()> {
    let mut interval = Interval::every(Duration::from_millis(50));
    let mut connection_pulse = Pulse::new(20); // Every 1 s
    let mut proxies_pulse = Pulse::new(100); //   Every 5 s + 0 tick
    let mut rules_pulse = Pulse::new(101); //     Every 5 s + 1 tick
    let mut version_pulse = Pulse::new(102); //   Every 5 s + 2 tick
    let mut config_pulse = Pulse::new(103); //    Every 5 s + 3 tick

    loop {
        if version_pulse.tick() {
            tx.send(Event::Update(UpdateEvent::Version(clash.get_version()?)))?;
        }
        if connection_pulse.tick() {
            tx.send(Event::Update(UpdateEvent::Connection(
                clash.get_connections()?.into(),
            )))?;
        }
        if rules_pulse.tick() {
            tx.send(Event::Update(UpdateEvent::Rules(clash.get_rules()?)))?;
        }
        if proxies_pulse.tick() {
            tx.send(Event::Update(UpdateEvent::Proxies(
                clash.get_proxies_with_providers()?,
            )))?;
        }
        if config_pulse.tick() {
            tx.send(Event::Update(UpdateEvent::Config(clash.get_configs()?)))?;
        }
        interval.tick();
    }
}

fn traffic_job(tx: Sender<Event>, clash: &Clash) -> TuiResult<()> {
    let mut traffics = clash.get_traffic()?;
    loop {
        match traffics.next() {
            Some(Ok(traffic)) => tx.send(Event::Update(UpdateEvent::Traffic(traffic)))?,
            Some(Err(e)) => warn!("{:?}", e),
            None => warn!("No more traffic"),
        }
    }
}

fn log_job(tx: Sender<Event>, clash: &Clash) -> TuiResult<()> {
    loop {
        let mut logs = clash.get_log()?;
        match logs.next() {
            Some(Ok(log)) => tx.send(Event::Update(UpdateEvent::Log(log)))?,
            Some(Err(e)) => warn!("{:?}", e),
            None => warn!("No more traffic"),
        }
    }
}

fn action_job(
    _opt: &TuiOpt,
    flags: &Flags,
    tx: Sender<Event>,
    rx: Receiver<Action>,
    clash: &Clash,
) -> TuiResult<()> {
    while let Ok(action) = rx.recv() {
        tx.send(Event::Action(action.clone()))?;
        match action {
            Action::TestLatency { proxies } => {
                let results = proxies
                    .par_iter()
                    .map(|proxy| {
                        clash
                            .get_proxy_delay(proxy, flags.test_url.as_str(), flags.timeout)
                            .map(|delay| delay.delay)
                    })
                    .collect::<Vec<_>>();

                let delays = results
                    .iter()
                    .filter_map(|result| result.as_ref().ok())
                    .copied()
                    .collect::<Vec<_>>();
                let failed = results.len() - delays.len();

                if failed != 0 {
                    warn!(
                        "   {}",
                        results
                            .into_iter()
                            .filter_map(|result| result.err())
                            .map(|error| error.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    warn!("({}) error(s) during test proxy delay", failed);
                }

                let average_delay =
                    (!delays.is_empty()).then(|| delays.iter().sum::<u64>() / delays.len() as u64);
                tx.send(Event::Update(UpdateEvent::ProxyTestLatencyDone {
                    succeeded: delays.len(),
                    failed,
                    average_delay,
                }))?;
                tx.send(Event::Update(UpdateEvent::Proxies(
                    clash.get_proxies_with_providers()?,
                )))?;
            }
            Action::ApplySelection { group, proxy } => {
                let _ = clash
                    .set_proxygroup_selected(&group, &proxy)
                    .map_err(|e| warn!("{:?}", e));
                tx.send(Event::Update(UpdateEvent::Proxies(
                    clash.get_proxies_with_providers()?,
                )))?;
            }
        }
    }
    Ok(())
}
