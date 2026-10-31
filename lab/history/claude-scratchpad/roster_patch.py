import sys

p = sys.argv[1]
s = open(p, encoding='utf-8').read()


def rep(o, n):
    global s
    assert s.count(o) == 1, (o[:80], s.count(o))
    s = s.replace(o, n)


rep("""                                                                                        family4_version = Some(version);
                                                                                        server_seq += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => warn!("      [conv] ⛔ loot grant delta REFUSED by the push accept-set: {e}"),
                                                                        }""", """                                                                                        family4_version = Some(version);
                                                                                        server_seq += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => warn!("      [conv] ⛔ loot grant delta REFUSED by the push accept-set: {e}"),
                                                                        }
                                                                        // The equipment change: the peer cards after the pickup,
                                                                        // as family-0/2 deltas (the loadout is read from 0/1).
                                                                        for (family, body) in roster_grant_deltas(acct) {
                                                                            match conv.push_with_sequence(d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &body) {
                                                                                Ok(push) => {
                                                                                    let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                    if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                        nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                        if sock.write_all(&f).await.is_ok() {
                                                                                            warn!("      ⭐⭐ LOOT GRANT: family {family} peer-card delta ({}B, {} object(s)), seq {server_seq}",
                                                                                                  body.len(), d1_bap::queuez_span_object_count(&body).unwrap_or(0));
                                                                                            server_seq += 1;
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(e) => warn!("      [conv] ⛔ family {family} grant delta REFUSED by the push accept-set: {e}"),
                                                                            }
                                                                        }""")
rep("""/// Read a hex-string payload from the environment (empty if unset/invalid).""", """/// `D1_LOOT_GRANT_ROSTER=<dir>`: the roster fixtures after a loot grant (`d1-oracle character
/// build-peer` output, `f<family>t<type>-<key>.bin`). Each family's type-1 card is pushed as a
/// version-1 delta on top of the version-0 roster baseline, so the client sees the new equipment.
fn roster_grant_deltas(root_soid: u64) -> Vec<(u32, Vec<u8>)> {
    let Some(dir) = std::env::var("D1_LOOT_GRANT_ROSTER").ok().filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(e) => {
            warn!("      ⛔ D1_LOOT_GRANT_ROSTER {dir}: {e}");
            return Vec::new();
        }
    };
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let name = f.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let parsed = name
            .strip_suffix(".bin")
            .and_then(|s| s.strip_prefix('f'))
            .and_then(|s| s.split_once('t'))
            .and_then(|(fam, rest)| rest.split_once('-').map(|(ty, key)| (fam, ty, key)))
            .and_then(|(fam, ty, key)| {
                Some((fam.parse::<u32>().ok()?, ty.parse::<u32>().ok()?, u64::from_str_radix(key, 16).ok()?))
            });
        let Some((fam, ty, key)) = parsed else { continue };
        if ty != 1 {
            continue;
        }
        let Some(&(_, instance)) = ROSTER_INSTANCE_TYPES.iter().find(|&&(k, _)| k == (fam, ty)) else {
            continue;
        };
        let Ok(payload) = std::fs::read(&f) else { continue };
        let mut b = d1_bap::queuez_family_header_versioned(fam, root_soid, 1, 1, d1_bap::QUEUEZ_FLAG_DELTA);
        b.extend_from_slice(&instance.to_be_bytes());
        b.extend_from_slice(&key.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(&payload);
        out.push((fam, b));
    }
    out
}

/// Read a hex-string payload from the environment (empty if unset/invalid).""")
open(p, 'w', encoding='utf-8', newline='').write(s)
print('patched')
