"""Exercise three independent native macOS pasteboards through three backend processes.

Build link_probe with --features diagnostics. No system clipboard or saved identity is used.
"""
import json
import os
import sys
import time
from test_two_hosts import Probe, digest, eventually


def main():
    if sys.platform != "darwin":
        raise SystemExit("This test needs independent named macOS pasteboards; use Rust tests on other systems.")
    peers = []
    try:
        for _ in range(3):
            peers.append(Probe([os.path.abspath("target/debug/examples/link_probe")]))
        for owner in peers:
            settings = owner.call("status")["settings"]
            settings["device_name"] = "同名电脑"
            owner.call("settings", settings=settings)
            for other in peers:
                if other is owner:
                    continue
                owner.call("trust_peer", certificate=other.identity["certificate"],
                           fingerprint=other.identity["fingerprint"], device_name="同名电脑",
                           address=other.identity["listen_address"])

        def ready():
            return all(len(p.call("status")["peers"]) == 2
                       and all(d["online"] and d["accepting"] for d in p.call("status")["peers"])
                       for p in peers)
        eventually(ready, "three independent processes connected")
        time.sleep(0.2)
        for owner in peers:
            for device in owner.call("status")["peers"]:
                device["settings"]["auto_send"] = True
                owner.call("configure_peer", noob_id=device["noob_id"], settings=device["settings"])
            settings = owner.call("status")["settings"]
            settings["mode"] = "Automatic"
            owner.call("settings", settings=settings)
            owner.collect()

        for index, sender in enumerate(peers):
            text = f"native three-device round {index} 中文 🦀\r\n"
            offsets = [len(p.events) for p in peers]
            sender.call("copy", text=text)
            eventually(lambda: all(p.call("read") == digest(text) for p in peers), "native fan-out")
            eventually(lambda: any(t["automatic"] and len(t["targets"]) == 2
                                   and all(d["state"] == "Applied" for d in t["targets"])
                                   for t in sender.call("status")["transfers"]), "per-target receipts")
            time.sleep(0.2)
            for p, offset in zip(peers, offsets):
                p.collect()
                sent_ids = {json.dumps(e["Transfer"]["id"], sort_keys=True)
                            for e in p.events[offset:] if "Transfer" in e}
                assert len(sent_ids) == (1 if p is sender else 0), "native clipboard echo"

        a, b, c = peers
        a.call("unpair", noob_id=b.identity["noob_id"])
        a.call("copy", text="after unpair")
        eventually(lambda: c.call("read") == digest("after unpair"), "remaining connection")
        time.sleep(0.2)
        assert b.call("read") != digest("after unpair"), "third device relayed remote text"
        assert len({p.identity["noob_id"] for p in peers}) == 3
        print(json.dumps({"passed": True, "processes": 3, "native_pasteboards": 3,
                          "fan_out_rounds": 3, "per_target_receipts": True,
                          "echo_suppressed": True, "unpair_isolated": True}))
    finally:
        for p in reversed(peers):
            p.close()


if __name__ == "__main__":
    main()
