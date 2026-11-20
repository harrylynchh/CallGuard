# CallGuard

Real-time scam screening for phone calls, aimed at older adults. Tufts senior
capstone (team Grandma's Perfume). This repository is the proof of concept for
the demo.

A caller dials the CallGuard Twilio number and hears a short screening
disclosure. The protected user's phone then rings, and a whisper says who is
calling. While they talk, a live web page shows a two-speaker transcript,
highlights scam phrases ("IRS", "gift card", "don't hang up") and keeps a
Low / Medium / High risk rating. **No audio is ever stored.** Audio exists
only in memory while it is forwarded to speech-to-text.

## How it works

```
caller ──► Twilio ──POST /voice──► CallGuard ──TwiML: disclosure, <Stream>, <Dial user>
                     │
                     └─ wss /media (both tracks, μ-law 8 kHz)
                              │
                     one task per call
                     ├─ inbound  (caller) ──► Deepgram stream #1 ─┐
                     └─ outbound (user)   ──► Deepgram stream #2 ─┤
                                                                  ▼
                                           detector (rules.toml) ──► SSE ──► live page
```

| Path | What it does |
| --- | --- |
| `server/src/twilio/` | Signature check middleware, form parsing, TwiML builders |
| `server/src/routes/` | HTTP routes: Twilio webhooks, `/media` WebSocket, live page, replay |
| `server/src/media/` | Media Streams messages, the per-call task, frame clock for latency |
| `server/src/deepgram/` | Deepgram live client and result parsing |
| `server/src/calls/` | In-memory call records, the events the page shows, detector glue |
| `server/src/replay/` | Plays `fixtures/scripts/*.txt` as a fake call |
| `server/src/bin/fake_twilio.rs` | Pretends to be Twilio Media Streams, for testing without a phone |
| `server/static/` | The two pages: one HTML file each, inline CSS/JS, no build step |
| `detector/` | Pure rules engine (no I/O) and `rules.toml` |

### Detection

`detector/rules.toml` defines each rule (id, description, phrases and/or regex
patterns, weight) plus the risk thresholds (Low < 3, Medium 3–5, High ≥ 6).

- Only the **caller's** speech is scored. The user's speech is shown but not scored.
- Each rule adds its weight **once per call**. Later matches are still shown as flags.
- Interim transcripts can raise a flag early. The final transcript for the same
  words doesn't flag again or double-count.

To tune detection, edit `rules.toml` and run `cargo test -p detector`. The fixture
tests check that both scam scripts reach High and the pharmacy call stays Low.

### Latency

Each flag reports `latency_ms`: the time from the arrival of the audio frame
holding the end of the matched words to the flag being emitted. For every
frame forwarded to Deepgram, the server records its position in that
Deepgram stream and when it arrived. Deepgram's word end times are then
mapped back to those arrival times. Replays have no audio, so their flags show
no latency.

## Running it

Requires stable Rust (`rustup`).

```sh
cp .env.example .env           # then fill it in (see below)
cargo run -p server            # http://localhost:3000
```

| Variable | Meaning |
| --- | --- |
| `TWILIO_ACCOUNT_SID` | Your Twilio account SID (used by the Twilio CLI) |
| `TWILIO_AUTH_TOKEN` | Used to verify `X-Twilio-Signature` |
| `TWILIO_NUMBER` | The CallGuard number, E.164 (e.g. `+16175550100`) |
| `USER_PHONE` | The protected user's phone, E.164 |
| `DEEPGRAM_API_KEY` | Optional. Without it calls still connect but aren't transcribed |
| `PUBLIC_URL` | Public HTTPS base URL of the tunnel, no trailing slash |
| `PORT` | Default `3000` |
| `SKIP_TWILIO_SIGNATURE` | `1` disables signature checks. **Local testing only.** |

Point the Twilio number at the server:

```sh
twilio phone-numbers:update $TWILIO_NUMBER --voice-url=$PUBLIC_URL/voice
```

### Without a phone

- **Replay mode** (the demo backup): open `http://localhost:3000` and click a
  script, or run `curl -X POST 'localhost:3000/replay?script=scam_irs'`. The
  script plays at about 150 words per minute through the same detector and
  live page as a real call.
- **Fake Twilio client**: streams both tracks to `/media` at real-time pace.
  It signs the request like Twilio does when `TWILIO_AUTH_TOKEN` and
  `PUBLIC_URL` are set.
  ```sh
  cargo run -p server --bin fake_twilio                       # generated tones
  cargo run -p server --bin fake_twilio -- --caller caller.ulaw --user user.ulaw
  # make audio files with: ffmpeg -i in.wav -ar 8000 -ac 1 -f mulaw caller.ulaw
  ```

## Development

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The server tests run without network access. A mock Deepgram server
(`server/tests/common/mock_deepgram.rs`) covers the full live path, from the
WebSocket through transcription and detection to the call record.
`TESTING.md` lists the checks that need a real phone, tunnel or API key.

## Safety rules

- Audio is never written to disk or logs. All call state is in memory and
  dropped when the call ends.
- Every Twilio request (`/voice`, `/whisper`, the `/media` upgrade) has its
  signature checked against a URL rebuilt from `PUBLIC_URL`.
- **Fail open:** if `/voice` can't parse a request, it still dials the user.
- **Loop guard:** a call from our own number is hung up.
- Out of scope for the PoC: carrier forwarding, databases, auth, SMS, ML,
  spoken warnings and deployment.
