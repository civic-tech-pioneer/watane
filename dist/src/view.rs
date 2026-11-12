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
