use crate::*;
pub(crate) fn derive(channel: PublicKey, entries: &[Entry]) -> ChannelView {
    let mut view = ChannelView {
        channel,
        fingerprint: channel.fingerprint(),
        name: None,
        base: None,
        head: entries.last().map(Entry::seq),
        releases: Vec::new(),
        bad: Vec::new(),
        status: Status::Ok,
        followed: false,
        index: false,
        listed_by: Vec::new(),
        listings: Vec::new(),
        last_success: None,
    };
    for entry in entries {
        let result: Result<(), String> = (|| {
            match entry.kind() {
                "profile" => {
                    view.name = Some(Profile::decode(entry.body()).map_err(str::to_owned)?.name);
                }
                "release" => {
                    let release = Release::decode(entry.body()).map_err(|e| e.to_string())?;
                    if let Some(old) = view.releases.iter().find(|r| {
                        r.release.slug == release.slug && r.release.version == release.version
                    }) {
                        return Err(format!(
                            "version {} of {} already released at seq {}",
                            release.version, release.slug, old.seq
                        ));
                    }
                    let reference = ReleaseRef {
                        publisher: PublisherKey::from_bytes(*channel.as_bytes()),
                        slug: release.slug.clone(),
                        version: release.version.clone(),
                    };
                    view.releases.push(ReleaseInfo {
                        reference,
                        channel,
                        entry: entry.id(),
                        seq: entry.seq(),
                        release,
                        withdrawn: None,
                    });
                }
                "withdraw" => {
                    let w = Withdraw::decode(entry.body()).map_err(str::to_owned)?;
                    let info = view
                        .releases
                        .iter_mut()
                        .find(|r| r.entry == w.release && r.seq < entry.seq())
                        .ok_or("withdraw names no earlier release")?;
                    if let Some(old) = &info.withdrawn {
                        return Err(format!("already withdrawn at seq {}", old.seq));
                    }
                    info.withdrawn = Some(Withdrawal {
                        seq: entry.seq(),
                        reason: w.reason,
                    });
                }
                "list" | "include" => {
                    let listing = Listing::decode(entry.body()).map_err(str::to_owned)?;
                    let include = entry.kind() == "include";
                    if include && listing.link.slug().is_some() {
                        return Err("include needs a channel link".into());
                    }
                    if !include && listing.link.slug().is_none() {
                        return Err("list needs a mod link".into());
                    }
                    if listing.link.channel() == channel {
                        return Err("lists its own channel".into());
                    }
                    if let Some(old) = view.listings.iter().find(|l| {
                        l.unlisted.is_none()
                            && l.include == include
                            && l.listing.link.channel() == listing.link.channel()
                            && l.listing.link.slug() == listing.link.slug()
                    }) {
                        return Err(format!("already listed at seq {}", old.seq));
                    }
                    view.listings.push(ListingInfo {
                        entry: entry.id(),
                        seq: entry.seq(),
                        include,
                        listing,
                        unlisted: None,
                    });
                }
                "unlist" => {
                    let id = body::decode_unlist(entry.body()).map_err(str::to_owned)?;
                    let info = view
                        .listings
                        .iter_mut()
                        .find(|l| l.entry == id && l.seq < entry.seq() && l.unlisted.is_none())
                        .ok_or("unlist names no live listing")?;
                    info.unlisted = Some(entry.seq());
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(cause) = result {
            view.bad.push(BadStatement {
                seq: entry.seq(),
                kind: entry.kind().into(),
                cause,
            });
        }
    }
    view
}
#[cfg(test)]
mod tests {
    use super::*;
    use watane_log::Log;
    #[test]
    fn ac4_derive_rules() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::open(&dir.path().join("log")).unwrap();
        let mut r = vec![
            0x8c, 1, 0x61, b'a', 0x65, b'1', b'.', b'0', b'.', b'0', 0x61, b'A', 0x61, b'S', 0x58,
            0x20,
        ];
        r.extend([0; 32]);
        r.extend([1, 0xf4, 0x80, 0x80, 0x80, 0xf6]);
        log.append("future", b"ignored").unwrap();
        log.append("profile", &[0x81, 0x61, b'A']).unwrap();
        log.append("profile", &[0x81, 0x61, b'B']).unwrap();
        log.append("profile", &[0x80]).unwrap();
        let release = log.append("release", &r).unwrap();
        log.append("release", &r).unwrap();
        let mut w = vec![0x82, 0x58, 0x20];
        w.extend([0x11; 32]);
        w.extend([0x63, b'b', b'u', b'g']);
        log.append("withdraw", &w).unwrap();
        w[3..35].copy_from_slice(release.id().as_bytes());
        log.append("withdraw", &w).unwrap();
        log.append("withdraw", &w).unwrap();
        log.append("withdraw", &[0x80]).unwrap();
        log.append("release", &[0x80]).unwrap();
        log.append("profile", &[0x81, 0x61, b' ']).unwrap();
        let entries = log.entries(&log.identity(), 0, usize::MAX).unwrap();
        let v = derive(log.identity(), &entries);
        assert_eq!(v.channel, log.identity());
        assert_eq!(v.head, Some(11));
        assert_eq!(v.name.as_deref(), Some("B"));
        assert_eq!(v.fingerprint, log.identity().to_string()[..10]);
        assert_eq!(v.base, None);
        assert_eq!(v.status, Status::Ok);
        assert!(!v.followed);
        assert_eq!(v.last_success, None);
        assert_eq!(v.releases.len(), 1);
        let info = &v.releases[0];
        assert_eq!(info.seq, 4);
        assert_eq!(info.channel, log.identity());
        assert_eq!(
            info.release,
            Release {
                slug: "a".parse().unwrap(),
                version: "1.0.0".parse().unwrap(),
                name: "A".into(),
                summary: "S".into(),
                package: PackageId::from_bytes([0; 32]),
                size: 1,
                source: false,
                offers: vec![],
                events: vec![],
                grants: vec![],
                origin: None
            }
        );
        assert_eq!(info.entry, release.id());
        assert_eq!(info.reference.slug.to_string(), "a");
        assert_eq!(info.reference.version.to_string(), "1.0.0");
        assert_eq!(
            info.reference.publisher.as_bytes(),
            log.identity().as_bytes()
        );
        assert_eq!(
            info.withdrawn,
            Some(Withdrawal {
                seq: 7,
                reason: "bug".into()
            })
        );
        assert_eq!(
            v.bad
                .iter()
                .map(|b| (b.seq, b.kind.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (3, "profile"),
                (5, "release"),
                (6, "withdraw"),
                (8, "withdraw"),
                (9, "withdraw"),
                (10, "release"),
                (11, "profile")
            ]
        );
        assert_eq!(
            v.bad[1].cause,
            "version 1.0.0 of a already released at seq 4"
        );
        assert_eq!(v.bad[2].cause, "withdraw names no earlier release");
        assert_eq!(v.bad[3].cause, "already withdrawn at seq 7");
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    #[test]
    fn ac3_derive_rules_step4() {
        let dir = tempfile::tempdir().unwrap();
        let log = watane_log::Log::open(&dir.path().join("log")).unwrap();
        let foreign = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        // Bodies constructed independently of the dist encoder.
        let listing = |key: &str, slug: &str, base: &str| {
            let link = format!("watane:{key}@https://{base}/{slug}");
            let mut b = vec![0x82, 0x78, link.len() as u8];
            b.extend(link.as_bytes());
            b.push(0x60);
            b
        };
        let m = listing(foreign, "#reading-list", "alice.example.org");
        let c = listing(foreign, "", "alice.example.org");
        log.append("list", &c).unwrap(); // 0 shape
        log.append("include", &m).unwrap(); // 1 shape
        log.append(
            "list",
            &listing(
                &log.identity().to_string(),
                "#reading-list",
                "own.example.org",
            ),
        )
        .unwrap(); // 2 own
        log.append(
            "include",
            &listing(&log.identity().to_string(), "", "own.example.org"),
        )
        .unwrap(); // 3 own
        let first = log.append("list", &m).unwrap(); // 4
        log.append(
            "list",
            &listing(foreign, "#reading-list", "mirror.example.org"),
        )
        .unwrap(); // 5 duplicate ignores base
        let inc = log.append("include", &c).unwrap(); // 6 different kind allowed
        log.append("include", &c).unwrap(); // 7 duplicate
        let unlist = |id: EntryId| {
            let mut b = vec![0x81, 0x58, 0x20];
            b.extend(id.as_bytes());
            b
        };
        log.append("unlist", &unlist(EntryId::from_bytes([0x22; 32])))
            .unwrap(); // 8 unknown
        log.append("unlist", &unlist(first.id())).unwrap(); // 9
        log.append("unlist", &unlist(first.id())).unwrap(); // 10 ended
        let again = log.append("list", &m).unwrap(); // 11 relist
        log.append("unlist", &unlist(inc.id())).unwrap(); // 12
        log.append("list", &[0x80]).unwrap(); // 13 bad body
        log.append("include", &[0x80]).unwrap(); // 14
        log.append("unlist", &[0x80]).unwrap(); // 15
        let entries = log.entries(&log.identity(), 0, usize::MAX).unwrap();
        let v = derive(log.identity(), &entries);
        assert!(!v.index);
        assert!(v.listed_by.is_empty());
        assert_eq!(
            v.listings
                .iter()
                .map(|l| (l.entry, l.seq, l.include, l.unlisted))
                .collect::<Vec<_>>(),
            vec![
                (first.id(), 4, false, Some(9)),
                (inc.id(), 6, true, Some(12)),
                (again.id(), 11, false, None)
            ]
        );
        assert_eq!(
            v.listings[0].listing.link.to_string(),
            format!("watane:{foreign}@https://alice.example.org/#reading-list")
        );
        assert_eq!(v.listings[0].listing.note, "");
        assert_eq!(
            v.bad
                .iter()
                .map(|b| (b.seq, b.kind.as_str(), b.cause.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (0, "list", "list needs a mod link"),
                (1, "include", "include needs a channel link"),
                (2, "list", "lists its own channel"),
                (3, "include", "lists its own channel"),
                (5, "list", "already listed at seq 4"),
                (7, "include", "already listed at seq 6"),
                (8, "unlist", "unlist names no live listing"),
                (10, "unlist", "unlist names no live listing"),
                (13, "list", "listing needs two fields"),
                (14, "include", "listing needs two fields"),
                (15, "unlist", "unlist needs one field")
            ]
        );
    }
}
