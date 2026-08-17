# Bitaxe + Braidpool Setup

> **Note:** This branch adds `--start-difficulty` and `--minimum-difficulty` CLI flags (not in upstream dev) specifically for real-hardware testing. Not intended for upstream merge.

**Hardware:** Bitaxe (BM1370 ASIC, Noctua fan upgrade)
**Status:** Testing

---

## Topology

```
Bitcoin Core (IPC)  ←→  Braidpool node  ←  Bitaxe (WiFi, SV1)
                              ↓
                         Dashboard (Vite dev server)
```

- **Node machine** — laptop: runs `bitcoin-node`, `braidpool/node`, dashboard
- **Bitaxe** — same WiFi subnet, stratum config points at laptop's LAN IP

---

## CLI flags

From `node/src/cli.rs`:

| Flag | Default | Controls |
|------|---------|---------|
| `--stratum-port` | `3333` | Stratum listener (what the Bitaxe connects to) |
| `--start-difficulty` | `1` | Initial difficulty for new connections *(this branch)* |
| `--minimum-difficulty` | `1` | Floor that clamps miner-suggested difficulty *(this branch)* |
| `--bind` | `0.0.0.0:6680` | P2P listener — NOT stratum |
| `--rpc-bind` | `127.0.0.1:6682` | JSON-RPC server |
| `--ipc-socket` | `/tmp/bitcoin-cpunet.sock` | Bitcoin Core IPC socket |
| `--network` | `mainnet` | Network (`cpunet`, `testnet4`, `signet`, `mainnet`) |

Stratum binds to `0.0.0.0:3333` by default — no extra flag needed for the Bitaxe to reach it from the network.

---

## SV1 vs SV2

Braidpool implements Stratum V1 only. Bitaxe (AxeOS) speaks SV1 natively — no compatibility gap.

---

## Step-by-step setup

### 1. Bitaxe hardware

- **Power**: 5V DC barrel jack. USB-C is data/flashing only on most Bitaxe variants.
- **Fan**: Noctua retrofit — confirm connector is seated.
- **OLED**: Optional, runs fine headless.

### 2. Bitaxe network config (AxeOS)

1. Power on — first boot creates WiFi AP (`Bitaxe_XXXX`).
2. Connect to that AP, navigate to `http://192.168.4.1`.
3. Set WiFi credentials (same network as node machine).
4. Find its DHCP IP via router admin or AxeOS status page.
5. In AxeOS → Stratum settings:
   ```
   Host: <node-machine-LAN-IP>
   Port: 3333
   User: bitaxe.worker1   (anything, not validated)
   Password: x
   ```

### 3. Bitcoin Core (IPC-enabled build)

```bash
cd <bitcoin-source-dir>
cmake -B build -DENABLE_IPC=ON
cmake --build build
```

Run on CPUnet:
```bash
cd build/bin
./bitcoin-node -cpunet -ipcbind=unix:/tmp/bitcoin-cpunet.sock -printtoconsole
```

Generate blocks (Braidpool skips notification if template is empty):
```bash
./bitcoin-cli -cpunet createwallet cpunet
./contrib/cpunet/miner --cli=./bitcoin-cli --ongoing \
  --address `./bitcoin-cli -cpunet getnewaddress` \
  --grind-cmd="./bitcoin-util -cpunet -ntasks=1 grind"
```

### 4. Braidpool node

For a Bitaxe (~400 GH/s), use a high difficulty to avoid flooding the handler.
Target ~10–30s per share. At 400 GH/s, difficulty `1_000_000` gives ~2.5s intervals:

```bash
cd braidpool/node
cargo run -- \
  --ipc-socket /tmp/bitcoin-cpunet.sock \
  --network cpunet \
  --start-difficulty 1000000 \
  --minimum-difficulty 1000000
```

`--minimum-difficulty` clamps AxeOS's `mining.suggest_difficulty` on connect — without it the miner's suggested value overrides `--start-difficulty`.

Tune live based on actual share rate seen in logs.

### 5. Dashboard

```bash
cd braidpool/dashboard
npm install
npm run dev
```

Dashboard endpoints (from `dashboard/src/URLs.ts`):
- `http://localhost:8999/api/v1` — Braidpool API
- `ws://localhost:5000` — main WebSocket
- `ws://localhost:65433/` — DAG WebSocket

### 6. Verify

```bash
cd braidpool-cli
cargo run -- gettips
```

Watch node logs for `"Miner connected"` then share submissions.

---

## What to observe

- **Share rate**: Check actual interval at the chosen difficulty and tune accordingly.
- **Unpadded nonce**: Does AxeOS send `"3"` or `"00000003"`? Check raw nonce values in submit logs — this is empirical confirmation for the `unpadded_nonce_not_rejected_by_length_check` test.
- **suggest_difficulty**: What value does AxeOS suggest on connect? Confirm the clamp works (node should respond with `max(suggested, minimum_difficulty)`).
- **payout_address gap**: Worker name isn't validated — typo fails silently.

---

## Issue log

*(fill in during session)*

- [ ] Confirm AxeOS version (affects AP SSID and UI layout)
- [ ] Confirm USB-C is data-only on this specific board
- [ ] Verify dashboard URL ports match running node
- [ ] File VARDIFF issue upstream if one doesn't exist
