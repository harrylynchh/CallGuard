# Live testing checklist

Everything here needs a real phone, tunnel or API key, so the automated tests
can't cover it. Work top to bottom. Each step assumes the ones before it pass.

## 0. Offline sanity check (no accounts needed)

- [ ] `cargo test --workspace` passes.
- [ ] `cargo run -p server` with `SKIP_TWILIO_SIGNATURE=1`, then in another terminal
      `cargo run -p server --bin fake_twilio`. The server logs `call ended … inbound_frames=300 outbound_frames=300`.
- [ ] Open `http://localhost:3000`, click **scam irs**. The page reaches **HIGH** and highlights phrases.
      Click **legit pharmacy**. It stays **LOW** with no flags.

## 1. Setup

- [ ] `cp .env.example .env` and fill in the real `TWILIO_*`, `USER_PHONE` and `DEEPGRAM_API_KEY`.
- [ ] Start a tunnel with a **fixed** domain to `localhost:3000` (e.g. `cloudflared` or an ngrok reserved domain).
- [ ] Set `PUBLIC_URL` to exactly the tunnel's `https://` URL, no trailing slash, and make sure `SKIP_TWILIO_SIGNATURE=0`.
- [ ] `cargo run -p server` and check `https://<tunnel>/health` returns `ok`.
- [ ] `twilio phone-numbers:update $TWILIO_NUMBER --voice-url=$PUBLIC_URL/voice`

## 2. Signatures (do this first; it's the most likely thing to break)

- [ ] Call the number. If the caller hears "an application error has occurred", look for
      `rejected request with missing or invalid Twilio signature` in the server log.
      That almost always means `PUBLIC_URL` doesn't exactly match the URL Twilio calls
      (`http` vs `https`, a trailing slash, a different hostname).
- [ ] The log shows `media stream started`, i.e. the `/media` upgrade also passed its signature check.
- [ ] `curl -X POST $PUBLIC_URL/voice` (unsigned) returns **403**.

## 3. Call flow

- [ ] Caller hears the disclosure: "This call is screened by CallGuard…"
- [ ] User's phone rings and shows the Twilio number as caller ID.
- [ ] On answering, the user (only) hears "Screened call from 6 1 7 …" before the call connects.
- [ ] Both sides can hear each other normally.
- [ ] Calling from `TWILIO_NUMBER` itself (or forwarding back to it) hangs up immediately (loop guard).

## 4. Transcription and detection

- [ ] `http://localhost:3000` lists the call as live. Open it.
- [ ] Caller speech appears on the left (CALLER) and user speech on the right (USER).
      If they're swapped, the inbound/outbound mapping is wrong for this setup.
- [ ] The disclosure TTS may appear as USER text at the start. That's expected (outbound track) and isn't scored.
- [ ] Read lines from `fixtures/scripts/scam_irs.txt` as the caller. The flags appear within about a second
      and the badge goes MEDIUM → HIGH. Write down the `ms` latencies shown on the flags for the report.
- [ ] Read `fixtures/scripts/legit_pharmacy.txt` as the caller on a new call. It stays **LOW**.
- [ ] When either side hangs up, the page shows "Call ended" and the log shows `call ended` with frame counts.
- [ ] Remove `DEEPGRAM_API_KEY`, restart and call. The call still connects normally, and the log warns
      `DEEPGRAM_API_KEY not set; skipping transcription`.

## 5. No audio stored

- [ ] After a call, `git status` shows no new files, and there are no `.ulaw`, `.wav` or similar files anywhere in the repo.
- [ ] The server log has no base64 payloads (look for long `//8…` strings).

## 6. Demo-day backup

- [ ] If the live call fails, open `http://localhost:3000` and click a replay. Same page, same detector.
- [ ] Projector check: the risk badge and transcript are readable from the back of the room.
