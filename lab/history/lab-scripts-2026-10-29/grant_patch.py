import sys

p = sys.argv[1]
s = open(p, encoding='utf-8').read()


def rep(o, n):
    global s
    assert s.count(o) == 1, (o[:80], s.count(o))
    s = s.replace(o, n)


rep("""                                    let mut family4_span: Option<Vec<u8>> = None;
                                    let mut family4_invalidated = false;
""", """                                    let mut family4_span: Option<Vec<u8>> = None;
                                    let mut family4_invalidated = false;
                                    // The family-4 version the client last accepted from us (the
                                    // baseline's, then each delta's): a delta must carry it + 1.
                                    let mut family4_version: Option<u32> = None;
                                    // 601 claims answered OK whose item still has to be delivered
                                    // (see `D1_LOOT_GRANT`), by nonce id.
                                    let mut loot_grants_due: Vec<u64> = Vec::new();
""")
rep("""                                                                            if family == d1_bap::QUEUEZ_FAMILY_ACCOUNT {
                                                                                family4_span = Some(body.clone());
                                                                            }""", """                                                                            if family == d1_bap::QUEUEZ_FAMILY_ACCOUNT {
                                                                                family4_version = d1_bap::queuez_span_version(&body);
                                                                                family4_span = Some(body.clone());
                                                                            }""")
rep("""                                                                                        if sock.write_all(&f).await.is_ok() {
                                                                                            family4_invalidated = true;
                                                                                            server_seq += 1;
                                                                                        }""", """                                                                                        if sock.write_all(&f).await.is_ok() {
                                                                                            family4_invalidated = true;
                                                                                            family4_version = Some(version);
                                                                                            server_seq += 1;
                                                                                        }""")
rep("""                                                                            Some(c) => warn!("      ⭐ LOOT CLAIM (601): source kind {} nonce {{{}, {:#x}}} word {}",
                                                                                             c.source_kind, c.key, c.id, c.word),""", """                                                                            Some(c) => {
                                                                                warn!("      ⭐ LOOT CLAIM (601): source kind {} nonce {{{}, {:#x}}} word {}",
                                                                                      c.source_kind, c.key, c.id, c.word);
                                                                                loot_grants_due.push(c.id);
                                                                            }""")
rep("""
                                                            if want_push
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)""", """
                                                            // ⭐ THE ITEM GRANT. After a 601 claim has been answered (the
                                                            // reply went out before this echo), hand the claimed item over
                                                            // as a family-4 delta carrying its 4/3 inventory element.
                                                            if !loot_grants_due.is_empty()
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)
                                                                && inner_type == Some(d1_bap::TYPE_ECHO_REQUEST)
                                                            {
                                                                if family4_version.is_none() {
                                                                    warn!("      ⛔ loot grant waits: no family-4 baseline accepted yet");
                                                                } else {
                                                                    for nonce in std::mem::take(&mut loot_grants_due) {
                                                                        let Some((soid, element)) = loot_grant_for(nonce) else {
                                                                            warn!("      ⛔ no D1_LOOT_GRANT entry for claimed nonce {nonce:#x}: nothing delivered");
                                                                            continue;
                                                                        };
                                                                        let version = family4_version.unwrap_or(0).wrapping_add(1);
                                                                        let acct = env_soid("D1_ACCOUNT_SOID", 0xD100_0000_0000_0001);
                                                                        let b = d1_bap::loot::item_grant_delta(acct, version, soid, &element);
                                                                        match conv.push_with_sequence(d1_bap::TYPE_QUEUEZ_OBJECT, server_seq, &b) {
                                                                            Ok(push) => {
                                                                                let sn = nonces.peek(d1_bap::Direction::ServerToClient);
                                                                                if let Ok(f) = d1_bap::seal_channel_frame(key, sn, &push) {
                                                                                    nonces.advance(d1_bap::Direction::ServerToClient);
                                                                                    if sock.write_all(&f).await.is_ok() {
                                                                                        warn!("      ⭐⭐ LOOT GRANT: nonce {nonce:#x} -> item {soid:#018x} ({}B element) as family-4 delta version {version}, seq {server_seq}",
                                                                                              element.len());
                                                                                        family4_version = Some(version);
                                                                                        server_seq += 1;
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => warn!("      [conv] ⛔ loot grant delta REFUSED by the push accept-set: {e}"),
                                                                        }
                                                                    }
                                                                }
                                                            }

                                                            if want_push
                                                                && conn_index == PRIMARY_BAP_CONNECTION.load(Ordering::SeqCst)""")
rep("""/// Read a hex-string payload from the environment (empty if unset/invalid).""", """/// `D1_LOOT_GRANT=NONCE=SOID:FILE[;…]`: the item a claimed world drop delivers. `NONCE` is the
/// pending-loot nonce id the 601 claim names, `SOID` the new item instance, `FILE` its 4/3 inventory
/// element as `d1-oracle object --tag 0x80801732` encodes it.
fn loot_grant_for(nonce: u64) -> Option<(u64, Vec<u8>)> {
    let spec = std::env::var("D1_LOOT_GRANT").ok()?;
    let num = |t: &str| -> Option<u64> {
        let t = t.trim();
        t.strip_prefix("0x").map_or_else(|| t.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
    };
    spec.split(';').find_map(|entry| {
        let (n, rest) = entry.split_once('=')?;
        let (soid, file) = rest.split_once(':')?;
        if num(n)? != nonce {
            return None;
        }
        match std::fs::read(file.trim()) {
            Ok(element) => Some((num(soid)?, element)),
            Err(e) => {
                warn!("      ⛔ D1_LOOT_GRANT {}: {e}", file.trim());
                None
            }
        }
    })
}

/// Read a hex-string payload from the environment (empty if unset/invalid).""")
open(p, 'w', encoding='utf-8', newline='').write(s)
print('patched')
