# ffwebapps — Performance Tuning

This page documents the opt-in performance knobs for heavy apps like Teams, WhatsApp, and Meet: hardware video decoding, process scheduling, software-rendering escape hatch, and the per-app memory caps — what each one writes and when to reach for it.

## Table of Contents

1. [Defaults first](#1-defaults-first)
2. [Hardware WebRTC](#2-hardware-webrtc)
3. [Software rendering](#3-software-rendering)
4. [The precedence rule](#4-the-precedence-rule)
5. [Scheduling](#5-scheduling)
6. [Memory caps](#6-memory-caps)
7. [Why there is no audio knob](#7-why-there-is-no-audio-knob)
8. [Verifying it works](#8-verifying-it-works)

---

## 1. Defaults first

The starting position is that on Linux, modern Firefox already does the right thing: it GPU-decodes regular video and the WebRTC H.264/VP9 paths by default, and the runtime's autoconfig reaffirms those defaults per app (`media.hardware-video-decoding.enabled`, `media.ffvpx-hw.enabled`) so they hold even if the user disabled hardware decoding globally. Firefox falls back to software automatically when no working VA-API driver is present.

So the knobs below are **opt-in** and **off by default**. They exist for the cases the defaults don't cover — forcing decode past a conservative GPU blocklist, the hardware VP8 path, real-time scheduling under load — and each can expose driver bugs, which is exactly why they aren't on for everyone. They are set per app at install or update time and written into the profile's `user.js` by `taskbartabs::write_profile_prefs`.

| Knob | `SiteConfig` field | Set with |
| --- | --- | --- |
| Hardware WebRTC | `hardware_webrtc` | `--hardware-webrtc` |
| Software rendering | `software_rendering` | `--software-rendering` |
| Scheduling | `scheduling` | `--scheduling <spec>` |

## 2. Hardware WebRTC

`--hardware-webrtc` maximises GPU video decoding for calls. Since Firefox already GPU-decodes the common paths, this flag flips the two things left off by default (`taskbartabs.rs:213-218`):

```javascript
user_pref("media.hardware-video-decoding.force-enabled", true);
user_pref("media.navigator.mediadatadecoder_vp8_hardware_enabled", true);
```

- **`force-enabled`** bypasses Firefox's GPU blocklist — the conservative deny-list that disables hardware decode on drivers Mozilla considers risky. Forcing past it is what helps when your driver actually works but isn't allowlisted.
- **The HW VP8 path** is the codec WhatsApp and Google Meet use for video; without this, VP8 calls decode on the CPU.

Both can expose driver bugs (that's why they're behind a flag and off the blocklist for a reason), so they are opt-in per app. They require a working VA-API driver; if there isn't one, Firefox falls back to software regardless.

## 3. Software rendering

`--software-rendering` is the opposite escape hatch: keep this app **entirely off the GPU**. It is for machines where the GPU driver itself is the source of hangs or lockups. It writes the full set of disables (`taskbartabs.rs:196-207`):

```javascript
user_pref("gfx.webrender.software", true);
user_pref("layers.acceleration.disabled", true);
user_pref("media.hardware-video-decoding.enabled", false);
user_pref("media.hardware-video-decoding.force-enabled", false);
user_pref("media.ffmpeg.vaapi.enabled", false);
user_pref("media.navigator.mediadatadecoder_vp8_hardware_enabled", false);
user_pref("media.navigator.mediadatadecoder_vp9_hardware_enabled", false);
user_pref("media.navigator.mediadatadecoder_h264_hardware_enabled", false);
```

This forces software WebRender compositing and software video decode and turns off every hardware decode path — VA-API, VP8, VP9, H.264. It trades CPU for stability: an app that crashed or corrupted under a flaky GPU driver runs slower but reliably.

## 4. The precedence rule

Because the two video knobs are opposites, the order in `write_profile_prefs` encodes a precedence: **`software_rendering` wins over `hardware_webrtc`.** The code is a literal `if / else if` (`taskbartabs.rs:196-218`):

```text
if software_rendering        → write the all-off block
else if hardware_webrtc      → write the force-on block
```

So if both are somehow set, software rendering takes effect and the WebRTC force-on block is never written. That matches intent: "this GPU is unstable" is a stronger statement than "push this GPU harder", and you would never want to force decode onto a GPU you just declared unusable.

## 5. Scheduling

Smooth audio and video under load is often a scheduling problem, not a decoding one — the runtime needs the CPU promptly when frames arrive. `--scheduling <spec>` runs the whole runtime under a chosen policy. The grammar (`scheduling_launcher`, `site.rs:151-174`):

| Spec | Launches under | Effect |
| --- | --- | --- |
| `nice:-5` | `nice -n -5` | Gentle priority bump; always works, no privileges needed |
| `rr:5` | `chrt -r 5` | `SCHED_RR` real-time at priority 5 |
| `fifo:5` | `chrt -f 5` | `SCHED_FIFO` real-time at priority 5 |
| `batch` | `chrt -b 0` | `SCHED_BATCH` — for non-interactive throughput |
| `idle` | `chrt -i 0` | `SCHED_IDLE` — lowest priority |

The real-time policies (`rr`, `fifo`) keep audio/video glitch-free under heavy system load but need `rtprio` privileges — e.g. membership in a group granted `rtprio` in `/etc/security/limits.conf`. The launcher wraps the runtime so a failed policy degrades gracefully rather than failing the launch:

```sh
<sched> "$@" || exec "$@"
```

If `chrt` can't apply the RT policy (no privilege), the `|| exec "$@"` still launches the app under normal scheduling. So `rr:5` is safe to set even on a machine that can't grant it — you just don't get the real-time benefit. The recommended combination for a heavy call app is forced hardware decode plus real-time scheduling:

```bash
ffwebapps site update <ULID> --hardware-webrtc true --scheduling rr:5
```

## 6. Memory caps

The runtime's autoconfig also tightens per-app memory, on the reasoning that a single-site app doesn't need general-browsing process counts (`_autoconfig.cfg:29-30`):

```javascript
defaultPref("dom.ipc.processCount", 4);
defaultPref("dom.ipc.processCount.webIsolated", 1);
```

These cap the content-process pools, cutting per-app memory. Fission still isolates cross-origin frames into their own (capped) processes, so security isolation is preserved. Importantly, these caps touch **content** processes only — the RDD (media decode), GPU, and socket processes that calls and video rely on are unaffected, so capping memory doesn't hurt call performance.

## 7. Why there is no audio knob

There is deliberately no hardware-audio option. WebRTC audio is Opus, which is CPU-cheap and has no GPU decode path to enable — a "hardware audio" pref would do nothing. Echo cancellation, noise suppression, and auto-gain are on by Firefox default. Under load, the lever that actually improves audio is `--scheduling` (RT), not a media pref, so that is the only audio-relevant knob ffwebapps exposes.

## 8. Verifying it works

The performance flags can silently fall back (no driver, no privilege), so confirm rather than assume:

- **Hardware decode:** open `about:support` and check Media → decoder, then `about:webrtc` *during* a call — inbound video should report a hardware decoder.
- **Scheduling:** the policy applied or fell back silently; check the runtime process with `chrt -p <pid>` to see whether the RT policy actually took.
- **Software rendering:** `about:support` should show WebRender (Software) and no hardware video decoder.

If a flag didn't take, the fallback is by design — the app still runs, just without the optimization. Re-applying needs only a relaunch, since `user.js` is rewritten on every launch from the stored config.
