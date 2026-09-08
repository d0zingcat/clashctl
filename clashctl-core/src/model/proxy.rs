use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
};

use serde::{Deserialize, Serialize};

use super::TimeType;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Proxies {
    pub proxies: HashMap<String, Proxy>,
}

impl Proxies {
    /// Add proxies returned by Mihomo proxy providers without replacing the
    /// canonical entries returned by `/proxies`.
    pub fn merge_proxy_providers(mut self, proxy_providers: ProxyProviders) -> Self {
        for provider in proxy_providers.providers.into_values() {
            for proxy in provider.proxies {
                self.proxies.entry(proxy.name).or_insert(proxy.proxy);
            }
        }
        self
    }

    pub fn normal(&self) -> impl Iterator<Item = (&String, &Proxy)> {
        self.iter().filter(|(_, x)| x.proxy_type.is_normal())
    }

    pub fn groups(&self) -> impl Iterator<Item = (&String, &Proxy)> {
        self.iter().filter(|(_, x)| x.proxy_type.is_group())
    }

    pub fn selectors(&self) -> impl Iterator<Item = (&String, &Proxy)> {
        self.iter().filter(|(_, x)| x.proxy_type.is_selector())
    }

    pub fn built_ins(&self) -> impl Iterator<Item = (&String, &Proxy)> {
        self.iter().filter(|(_, x)| x.proxy_type.is_built_in())
    }
}

/// Response returned by Mihomo's `/providers/proxies` endpoint.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyProviders {
    /// A `BTreeMap` makes duplicate names across providers resolve in a
    /// deterministic provider order when merged.
    pub providers: BTreeMap<String, ProxyProvider>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyProvider {
    pub proxies: Vec<ProviderProxy>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProviderProxy {
    pub name: String,
    #[serde(flatten)]
    pub proxy: Proxy,
}

impl Deref for Proxies {
    type Target = HashMap<String, Proxy>;

    fn deref(&self) -> &Self::Target {
        &self.proxies
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Proxy {
    #[serde(rename = "type")]
    pub proxy_type: ProxyType,
    pub history: Vec<History>,
    pub udp: Option<bool>,

    // Only present in ProxyGroups
    pub all: Option<Vec<String>>,
    pub now: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct History {
    pub time: TimeType,
    pub delay: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
#[cfg_attr(
    feature = "enum_ext",
    derive(strum::EnumString, strum::Display, strum::EnumVariantNames),
    strum(ascii_case_insensitive)
)]
pub enum ProxyType {
    // Built-In types
    Direct,
    Reject,
    // ProxyGroups
    Selector,
    URLTest,
    Fallback,
    LoadBalance,
    // Proxies
    Shadowsocks,
    Vmess,
    ShadowsocksR,
    Http,
    Snell,
    Trojan,
    Socks5,
    // Relay
    Relay,
    // Unknown
    #[serde(other)]
    Unknown,
}

impl ProxyType {
    pub fn is_selector(&self) -> bool {
        matches!(self, ProxyType::Selector)
    }

    pub fn is_group(&self) -> bool {
        matches!(
            self,
            ProxyType::Selector
                | ProxyType::URLTest
                | ProxyType::Fallback
                | ProxyType::LoadBalance
                | ProxyType::Relay
        )
    }

    pub fn is_built_in(&self) -> bool {
        matches!(self, ProxyType::Direct | ProxyType::Reject)
    }

    pub fn is_normal(&self) -> bool {
        matches!(
            self,
            ProxyType::Shadowsocks
                | ProxyType::Vmess
                | ProxyType::ShadowsocksR
                | ProxyType::Http
                | ProxyType::Snell
                | ProxyType::Trojan
                | ProxyType::Socks5
        )
    }
}

#[test]
fn test_proxies() {
    let proxy_kv = [
        (
            "test_a".to_owned(),
            Proxy {
                proxy_type: ProxyType::Direct,
                history: vec![],
                udp: Some(false),
                all: None,
                now: None,
            },
        ),
        (
            "test_b".to_owned(),
            Proxy {
                proxy_type: ProxyType::Selector,
                history: vec![],
                udp: Some(false),
                all: Some(vec!["test_c".into()]),
                now: Some("test_c".into()),
            },
        ),
        (
            "test_c".to_owned(),
            Proxy {
                proxy_type: ProxyType::Shadowsocks,
                history: vec![],
                udp: Some(false),
                all: None,
                now: None,
            },
        ),
        (
            "test_d".to_owned(),
            Proxy {
                proxy_type: ProxyType::Fallback,
                history: vec![],
                udp: Some(false),
                all: Some(vec!["test_c".into()]),
                now: Some("test_c".into()),
            },
        ),
    ];
    let proxies = Proxies {
        proxies: HashMap::from(proxy_kv),
    };
    assert_eq!(
        {
            let mut tmp = proxies.groups().map(|x| x.0).collect::<Vec<_>>();
            tmp.sort();
            tmp
        },
        vec!["test_b", "test_d"]
    );
    assert_eq!(
        proxies.built_ins().map(|x| x.0).collect::<Vec<_>>(),
        vec!["test_a"]
    );
    assert_eq!(
        proxies.normal().map(|x| x.0).collect::<Vec<_>>(),
        vec!["test_c"]
    );
}

#[cfg(test)]
mod provider_tests {
    use serde_json::from_str;

    use super::*;

    fn proxy(proxy_type: ProxyType) -> Proxy {
        Proxy {
            proxy_type,
            history: vec![],
            udp: None,
            all: None,
            now: None,
        }
    }

    #[test]
    fn deserializes_and_merges_provider_only_proxies() {
        let providers: ProxyProviders = from_str(
            r#"{"providers":{"subscription":{"proxies":[{"name":"provider-node","type":"Shadowsocks","history":[]}]}}}"#,
        )
        .unwrap();
        let merged = Proxies::default().merge_proxy_providers(providers);

        assert_eq!(merged["provider-node"].proxy_type, ProxyType::Shadowsocks);
    }

    #[test]
    fn top_level_proxy_takes_precedence_over_provider_proxy() {
        let providers = ProxyProviders {
            providers: BTreeMap::from([(
                "subscription".to_owned(),
                ProxyProvider {
                    proxies: vec![ProviderProxy {
                        name: "duplicate".to_owned(),
                        proxy: proxy(ProxyType::Shadowsocks),
                    }],
                },
            )]),
        };
        let top_level = Proxies {
            proxies: HashMap::from([("duplicate".to_owned(), proxy(ProxyType::Direct))]),
        };

        let merged = top_level.merge_proxy_providers(providers);

        assert_eq!(merged["duplicate"].proxy_type, ProxyType::Direct);
    }

    #[test]
    fn first_provider_in_deterministic_order_wins_duplicate_names() {
        let providers = ProxyProviders {
            providers: BTreeMap::from([
                (
                    "a-provider".to_owned(),
                    ProxyProvider {
                        proxies: vec![ProviderProxy {
                            name: "duplicate".to_owned(),
                            proxy: proxy(ProxyType::Shadowsocks),
                        }],
                    },
                ),
                (
                    "z-provider".to_owned(),
                    ProxyProvider {
                        proxies: vec![ProviderProxy {
                            name: "duplicate".to_owned(),
                            proxy: proxy(ProxyType::Trojan),
                        }],
                    },
                ),
            ]),
        };

        let merged = Proxies::default().merge_proxy_providers(providers);

        assert_eq!(merged["duplicate"].proxy_type, ProxyType::Shadowsocks);
    }

    #[test]
    fn empty_provider_response_preserves_proxy_map() {
        let top_level = Proxies {
            proxies: HashMap::from([("node".to_owned(), proxy(ProxyType::Trojan))]),
        };

        assert_eq!(
            top_level
                .clone()
                .merge_proxy_providers(ProxyProviders::default()),
            top_level
        );
    }
}
