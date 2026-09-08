use std::{cmp::Ordering, collections::HashMap, fmt::Debug, marker::PhantomData};

use clashctl_core::model::Proxies;
use crossterm::event::KeyCode;
use tui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::{
    components::{Footer, FooterItem, MovableListManage, ProxyGroup, ProxyItem},
    interactive::{EndlessSelf, ProxySort, Sortable},
    ui::{Action, Coord, ListEvent, Wrap, help_footer, tagged_footer},
};

// TODO Proxy tree furthur functions
//
// - [X] Right & Enter can be used to apply selection
// - [X] Esc for exist expand mode
// - [X] T for test latency of current group
// - [X] S for switch between sorting strategies
// - [ ] / for searching
//
// In order for functions to be implemented, these are required:
// - Remove Enter from InterfaceEvent::ToggleHold
// - Maybe a new InterfaceEvent::Confirm correstponds to Enter
// - `T`, `S`, `/` in proxy event handling
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyTree<'a> {
    pub(super) groups: Vec<ProxyGroup<'a>>,
    pub(super) expanded: bool,
    pub(super) cursor: usize,
    pub(super) testing: bool,
    pub(super) footer: Footer<'a>,
    sort_method: ProxySort,
}

impl<'a> Default for ProxyTree<'a> {
    fn default() -> Self {
        let mut ret = Self {
            groups: Default::default(),
            expanded: Default::default(),
            cursor: Default::default(),
            footer: Default::default(),
            testing: Default::default(),
            sort_method: Default::default(),
        };
        ret.update_footer();
        ret
    }
}

impl<'a> ProxyTree<'a> {
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[inline]
    pub fn current_group(&self) -> &ProxyGroup {
        &self.groups[self.cursor]
    }

    #[inline]
    pub fn is_testing(&self) -> bool {
        self.testing
    }

    /// Return the proxies targeted by a manual latency test.
    ///
    /// When a group is expanded, test only the highlighted node. Otherwise,
    /// test every regular proxy in the focused group.
    pub fn manual_latency_test_targets(&self) -> Vec<String> {
        let Some(group) = self.groups.get(self.cursor) else {
            return vec![];
        };

        if self.expanded {
            return group
                .members
                .get(group.cursor)
                .filter(|proxy| proxy.proxy_type.is_normal())
                .map(|proxy| vec![proxy.name.clone()])
                .unwrap_or_default();
        }

        group
            .members
            .iter()
            .filter(|proxy| proxy.proxy_type.is_normal())
            .map(|proxy| proxy.name.clone())
            .collect()
    }

    #[inline]
    pub fn start_testing(&mut self) -> &mut Self {
        self.testing = true;
        self.update_footer()
    }

    #[inline]
    pub fn end_testing(&mut self) -> &mut Self {
        self.testing = false;
        self.update_footer()
    }

    pub fn sort_groups_with_frequency(&mut self, freq: &HashMap<String, usize>) -> &mut Self {
        self.groups
            .sort_by(|a, b| match (freq.get(&a.name), freq.get(&b.name)) {
                (Some(a_freq), Some(b_freq)) => b_freq.cmp(a_freq),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            });
        self
    }

    pub fn update_footer(&mut self) -> &mut Self {
        let mut footer = Footer::default();
        let current_group = match self.groups.get(self.cursor) {
            Some(grp) => grp,
            _ => return self,
        };

        if !self.expanded {
            let group_name = current_group.name.clone();
            let style = Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::REVERSED);

            let highlight = style.add_modifier(Modifier::BOLD);
            let sort = tagged_footer("Sort", style, self.sort_method);

            let mut left = vec![
                FooterItem::span(Span::styled(" FREE ", style)),
                FooterItem::span(Span::styled(" SPACE to expand ", style)),
                if self.testing {
                    FooterItem::span(Span::styled(" Testing ", highlight.fg(Color::Green)))
                } else {
                    FooterItem::spans(help_footer("Test group", style, highlight)).wrapped()
                },
                FooterItem::spans(sort),
            ];

            footer.append_left(&mut left);

            let name = FooterItem::span(Span::styled(group_name, style)).wrapped();
            footer.push_right(name);

            if let Some(now) = current_group.current {
                footer.push_right(
                    FooterItem::span(Span::raw(current_group.members[now].name.to_owned()))
                        .wrapped(),
                );
            }
        } else {
            let style = Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::REVERSED);
            let highlight = style.add_modifier(Modifier::BOLD);

            footer.push_left(FooterItem::span(Span::styled(" [^] ▲ ▼ Move ", style)));

            if current_group.proxy_type.is_selector() {
                footer.push_left(FooterItem::span(Span::styled(" ▶ Select ", style)));
            }

            footer.push_left(if self.testing {
                FooterItem::span(Span::styled(" Testing ", highlight.fg(Color::Blue)))
            } else {
                FooterItem::spans(help_footer("Test node", style, highlight)).wrapped()
            });

            footer.push_left(tagged_footer("Sort", style, self.sort_method).into());

            if let Some(ref now) = current_group.members[current_group.cursor].now {
                footer.push_right(FooterItem::span(Span::raw(now.to_owned())).wrapped());
            }
        }
        self.footer = footer;
        self
    }

    pub fn replace_with(&mut self, mut new_tree: ProxyTree<'a>) -> &mut Self {
        // let map = HashMap::<_, _,
        // RandomState>::from_iter(self.groups.iter().map(|x|
        // (&x.name, x)));
        let old_groups = &self.groups;
        let current_group = self.groups.get(self.cursor);
        for (index, new_group) in new_tree.groups.iter_mut().enumerate() {
            if let Some(true) = current_group.map(|x| x.name == new_group.name) {
                new_tree.cursor = index;
            }
            if let Some(old_group) = old_groups.iter().find(|group| group.name == new_group.name) {
                new_group.cursor = old_group
                    .members
                    .get(old_group.cursor)
                    .and_then(|old_member| {
                        new_group
                            .members
                            .iter()
                            .position(|new_member| new_member.name == old_member.name)
                    })
                    .or(new_group.current)
                    .unwrap_or_default()
            }
        }
        self.groups = new_tree.groups;
        let method = self.sort_method;
        self.sort_with(&method);
        self.update_footer()
    }
}

impl<'a> From<Proxies> for ProxyTree<'a> {
    fn from(val: Proxies) -> Self {
        let mut ret = Self {
            groups: Vec::with_capacity(val.len()),
            ..Default::default()
        };
        for (name, group) in val.groups() {
            let all = group
                .all
                .as_ref()
                .expect("ProxyGroup should have member vec");
            let mut members = Vec::with_capacity(all.len());
            for x in all.iter() {
                if let Some(proxy) = val.get(x) {
                    members.push((x.as_str(), proxy).into());
                }
            }

            if members.is_empty() {
                continue;
            }

            let current = group.now.as_ref().and_then(|name| {
                members
                    .iter()
                    .position(|item: &ProxyItem| &item.name == name)
            });

            ret.groups.push(ProxyGroup {
                _life: PhantomData,
                name: name.to_owned(),
                proxy_type: group.proxy_type,
                cursor: current.unwrap_or_default(),
                current,
                members,
            })
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use clashctl_core::model::{ProviderProxy, Proxy, ProxyProvider, ProxyProviders, ProxyType};
    use crossterm::event::KeyCode;

    use super::*;

    fn proxy(proxy_type: ProxyType, all: Option<Vec<&str>>, now: Option<&str>) -> Proxy {
        Proxy {
            proxy_type,
            history: vec![],
            udp: None,
            all: all.map(|members| members.into_iter().map(str::to_owned).collect()),
            now: now.map(str::to_owned),
        }
    }

    fn proxies(entries: Vec<(&str, Proxy)>) -> Proxies {
        Proxies {
            proxies: entries
                .into_iter()
                .map(|(name, proxy)| (name.to_owned(), proxy))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn skips_members_missing_from_proxy_map() {
        let tree = ProxyTree::from(proxies(vec![
            (
                "Group",
                proxy(
                    ProxyType::Selector,
                    Some(vec!["valid", "missing"]),
                    Some("valid"),
                ),
            ),
            ("valid", proxy(ProxyType::Shadowsocks, None, None)),
        ]));

        assert_eq!(tree.groups.len(), 1);
        assert_eq!(tree.groups[0].members.len(), 1);
        assert_eq!(tree.groups[0].members[0].name, "valid");
        assert_eq!(tree.groups[0].current, Some(0));
    }

    #[test]
    fn clears_current_when_current_member_is_missing() {
        let tree = ProxyTree::from(proxies(vec![
            (
                "Group",
                proxy(ProxyType::Selector, Some(vec!["valid"]), Some("missing")),
            ),
            ("valid", proxy(ProxyType::Shadowsocks, None, None)),
        ]));

        assert_eq!(tree.groups[0].current, None);
        assert_eq!(tree.groups[0].cursor, 0);
    }

    #[test]
    fn skips_groups_with_no_available_members() {
        let tree = ProxyTree::from(proxies(vec![(
            "Empty",
            proxy(ProxyType::Selector, Some(vec!["missing"]), Some("missing")),
        )]));

        assert!(tree.groups.is_empty());
    }

    #[test]
    fn keeps_valid_groups_selectable() {
        let mut tree = ProxyTree::from(proxies(vec![
            (
                "Group",
                proxy(ProxyType::Selector, Some(vec!["one", "two"]), Some("one")),
            ),
            ("one", proxy(ProxyType::Shadowsocks, None, None)),
            ("two", proxy(ProxyType::Trojan, None, None)),
        ]));

        tree.hold();
        tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Down,
        });
        let action = tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Enter,
        });

        assert!(matches!(
            action,
            Some(Action::ApplySelection { group, proxy }) if group == "Group" && proxy == "two"
        ));
    }

    #[test]
    fn manual_latency_test_targets_follow_the_current_focus() {
        let mut tree = ProxyTree::from(proxies(vec![
            (
                "Group",
                proxy(
                    ProxyType::Selector,
                    Some(vec!["one", "builtin", "two"]),
                    Some("one"),
                ),
            ),
            ("one", proxy(ProxyType::Shadowsocks, None, None)),
            ("builtin", proxy(ProxyType::Direct, None, None)),
            ("two", proxy(ProxyType::Trojan, None, None)),
        ]));

        assert_eq!(
            tree.manual_latency_test_targets(),
            vec!["one".to_owned(), "two".to_owned()]
        );

        tree.hold();
        tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Down,
        });
        assert!(tree.manual_latency_test_targets().is_empty());

        tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Down,
        });
        assert_eq!(tree.manual_latency_test_targets(), vec!["two".to_owned()]);
    }

    #[test]
    fn provider_resolved_group_keeps_current_member_and_can_switch() {
        let top_level = proxies(vec![(
            "Group",
            proxy(
                ProxyType::Selector,
                Some(vec!["provider-one", "provider-two"]),
                Some("provider-one"),
            ),
        )]);
        let providers = ProxyProviders {
            providers: BTreeMap::from([(
                "subscription".to_owned(),
                ProxyProvider {
                    proxies: vec![
                        ProviderProxy {
                            name: "provider-one".to_owned(),
                            proxy: proxy(ProxyType::Shadowsocks, None, None),
                        },
                        ProviderProxy {
                            name: "provider-two".to_owned(),
                            proxy: proxy(ProxyType::Trojan, None, None),
                        },
                    ],
                },
            )]),
        };
        let mut tree = ProxyTree::from(top_level.merge_proxy_providers(providers));

        assert_eq!(tree.groups.len(), 1);
        assert_eq!(tree.groups[0].current, Some(0));
        assert_eq!(tree.groups[0].members.len(), 2);

        tree.hold();
        tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Down,
        });
        let action = tree.handle(ListEvent {
            fast: false,
            code: KeyCode::Enter,
        });

        assert!(matches!(
            action,
            Some(Action::ApplySelection { group, proxy }) if group == "Group" && proxy == "provider-two"
        ));
    }
}

impl<'a> MovableListManage for ProxyTree<'a> {
    fn sort(&mut self) -> &mut Self {
        let method = self.sort_method;
        self.sort_with(&method);
        self
    }

    fn next_sort(&mut self) -> &mut Self {
        self.sort_method.next_self();
        let method = self.sort_method;
        self.sort_with(&method);
        self.update_footer()
    }

    fn prev_sort(&mut self) -> &mut Self {
        self.sort_method.prev_self();
        let method = self.sort_method;
        self.sort_with(&method);
        self.update_footer()
    }

    fn current_pos(&self) -> Coord {
        Default::default()
    }

    #[inline]
    fn toggle(&mut self) -> &mut Self {
        self.expanded = !self.expanded;
        self.update_footer()
    }

    #[inline]
    fn end(&mut self) -> &mut Self {
        self.expanded = false;
        self.update_footer()
    }

    #[inline]
    fn len(&self) -> usize {
        self.groups.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    fn hold(&mut self) -> &mut Self {
        self.expanded = true;
        self
    }

    fn handle(&mut self, event: ListEvent) -> Option<Action> {
        if self.expanded {
            let step = if event.fast { 3 } else { 1 };
            let group = &mut self.groups[self.cursor];
            match event.code {
                KeyCode::Up => {
                    if group.cursor > 0 {
                        group.cursor = group.cursor.saturating_sub(step)
                    }
                }
                KeyCode::Down => {
                    let left = group.members.len().saturating_sub(group.cursor + 1);

                    group.cursor += left.min(step)
                }
                KeyCode::Right | KeyCode::Enter => {
                    if group.proxy_type.is_selector() {
                        let current = group.members[group.cursor].name.to_owned();
                        return Some(Action::ApplySelection {
                            group: group.name.to_owned(),
                            proxy: current,
                        });
                    }
                }
                _ => {}
            }
        } else {
            match event.code {
                KeyCode::Up => {
                    if self.cursor > 0 {
                        self.cursor = self.cursor.saturating_sub(1)
                    }
                }
                KeyCode::Down => {
                    if self.cursor < self.groups.len() - 1 {
                        self.cursor = self.cursor.saturating_add(1)
                    }
                }
                KeyCode::Enter => self.expanded = true,
                _ => {}
            }
        }
        self.update_footer();
        None
    }

    fn offset(&self) -> &crate::Coord {
        &Coord {
            x: 0,
            y: 0,
            hold: false,
        }
    }
}
