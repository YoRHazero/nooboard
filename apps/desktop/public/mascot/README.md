# Bird model and business animations

## Q bird brand assets

The approved Q bird illustration is preserved at `apps/desktop/assets/brand/q-bird.png`, with its image-generation prompt alongside it. It is based on the current cream-colored Blender bird. Run `npm run icons:generate` from `apps/desktop` to resize and encode this exact artwork using the installed Tauri CLI. The command updates the shared 256 px `public/mascot/icon.png` and the PNG, macOS ICNS and Windows ICO files in `src-tauri/icons`; it does not redraw or crop the character. Mobile outputs are discarded because this app targets desktops.

The shared transparent icon appears in the startup screen, sidebar, device identity, discovery and pairing dialogs, clipboard panel, and browser favicon. Native window, installer, Dock/taskbar and tray icons use the corresponding bundle assets. macOS uses the icon's alpha silhouette as its menu-bar template; Windows retains the color artwork. Startup breathing stops for connection errors and respects both the operating system and application reduced-motion preferences. Theme and motion preferences apply before the first desktop snapshot, including the startup screen.

## 3D scene

The default home stage loads the complete `bird-business-19.glb` scene, including the approved bird, board, mailbox and supported paper. It uses authoritative snapshot events through `ActivityCues → Playback → ClipPlayer → BirdScene`. Playback never changes clipboard contents or transfer outcomes.

Open `?mascot=business-19`, or choose **Inspect animations / 动画检查** in the browser footer, to inspect all twelve clips manually. The earlier `?mascot=motion-16` URL is an alias. The pose slider stops the selected clip at a chosen percentage; click its button to replay. **Live stage / 业务联动** returns to normal event-driven playback. This review mode does not emit business events.

| Authoritative change | Performance |
| --- | --- |
| Local text copied | capture |
| Text send activity begins | send |
| Incoming text applied | receive_text |
| Incoming files/image finished as Completed or Saved | receive_file |
| Receipt for the relevant send confirms Applied/Completed | success, after the send performance finishes |
| Relevant rejected, failed or unconfirmed receipt | error; cancellation stays quiet |
| Synchronization paused / all peers offline / resumed or reconnected | paused / offline / resume |
| Bird hovered or keyboard-focused / clicked | curious / hop |

File transfer start and byte completion do not trigger receipt animation. Saved remains a partial business outcome even though the file can be inspected. Receipt correlation uses the activity ID for text and the task key for content transfers, including a receipt delivered in the same snapshot as the send activity. Newer activity invalidates a pending cosmetic receipt. Busy events remain in the mailbox without building an animation queue; gesture and wake animations yield to business activity. A carried-paper performance finishes placing its props before entering pause/offline, while the actual business mode changes immediately. Hidden windows reset playback and suspend rendering. Recovery and initial snapshots never replay old events.

## Source and export

The editable source is `.develop_doc/design/bird-business-19/NOOBOARD-business-19.blend` at the repository root (local design archive). It retains version 18's approved shape, wing material, modifiers and three base motions. The legacy board/mailbox choreography is appended as a reference; its hidden rig drives body/travel timing. New head gestures, foot lift, wing balance, eyelid poses and paper/door constraints were edited in Blender's native UI. No script regenerates the bird, poses or geometry.

Export from Blender as **glTF Binary (.glb)**, **Visible Objects**, **Active Scene**, **Data → Mesh → Apply Modifiers on**, animation mode **Scene**, **Split by Object off**, **Sample Animations on**, at 30 fps, scene range 1–1700. Exclude cameras and lights. Keep the legacy reference meshes hidden. Apply Modifiers preserves the smooth evaluated wing, beak, crest and tail in the app; the source retains editable modifiers.

From `apps/desktop`:

```sh
node scripts/package-bird-motion.mjs ../../.develop_doc/design/bird-business-19/NOOBOARD-business-19.glb public/mascot/bird-business-19.glb --business
```

The packager copies Blender's baked samples into named clips and adjusts timestamps. Base motions and resume play at 30 fps; the appended business timing plays at its original 24 fps. Meshes, materials, skin weights and sampled transforms are preserved. Every clip retains all tracks, including constants, so switching clips resets props and expressions. The packaging step normalizes `Bird_Root.001` to the runtime interaction name `Bird_Root` and excludes the studio floor and unused helper from scene roots.

For animation-only iteration, export the affected scene range with **Set all animations starting at 0 off**, then pass `--replace-clips=<name>` against an existing packaged asset. Nodes map by name; unchanged clips and geometry remain intact. Blender may omit tracks that are constant over the limited range; the packager retains their existing samples only after checking that Blender's exported static transform agrees. For a focused native edit, add `--replace-nodes="exact Blender node name"` to copy only that node’s sampled tracks and retain every other track verbatim. This avoids replacing unrelated constant tracks with Blender’s rest-frame values. Restore the full scene range before saving the source.

The archived full export includes the receiving handoff correction, board approach and tray rails. Its final receiving retreat is superseded by the native 880–1078 frame export `NOOBOARD-business-19-board-exit.glb`. To reproduce the shipped asset from these exports, run the full packaging command above, then:

```sh
node scripts/package-bird-motion.mjs ../../.develop_doc/design/bird-business-19/NOOBOARD-business-19-board-exit.glb public/mascot/bird-business-19.glb --business --replace-clips=receive_text
```

The file-receipt start is additionally corrected in `NOOBOARD-business-19-file-start.glb` (1079–1215). It contains a native edit to the clearance parent only. Apply it after the preceding commands:

```sh
node scripts/package-bird-motion.mjs ../../.develop_doc/design/bird-business-19/NOOBOARD-business-19-file-start.glb public/mascot/bird-business-19.glb --business --replace-clips=receive_file '--replace-nodes=20 - Board clearance'
```

The editable Blender source was saved with all corrections and the full 1–1700 range before the partial export. A fresh full export from that source needs only the full packaging command. The older `NOOBOARD-business-19-receive-fix.glb` is historical and must not be applied over these exports.

## Performances

| Clip | Inclusive Blender frames | Duration | Performance |
| --- | --- | --- | --- |
| idle | 1–121 | 4 s | Breathing and blinking |
| curious | 141–241 | 3.333 s | Curious head turn |
| hop | 261–360 | 3.3 s | Anticipation, hop and settle |
| capture | 546–613 | 2.792 s | Look down, lift foot at the board, settle |
| send | 687–879 | 8 s | Take paper from the board, carry it to the closed mailbox, turn it flat and insert at the slot, return |
| receive_text | 880–1078 | 8.25 s | Open mailbox, extend the paper tray, take the flat paper, carry it to the board and place it |
| receive_file | 1079–1215 | 5.667 s | Open mailbox and inspect the file; leave it inside and return empty-beaked |
| success | 1344–1386 | 1.75 s | Small affirmative nod |
| error | 1387–1447 | 2.5 s | Hesitation and puzzled head tilt |
| paused | 1448–1520 | 3 s | Settle and close eyes; hold the final pose |
| offline | 1521–1617 | 4 s | Remain awake and wait; loop |
| resume | 1618–1678 | 2 s | Open eyes, lift head and return to neutral |

Send and success are distinct performances. Animation completion is not proof of delivery. Files stay associated with the mailbox; only clipboard content is carried back to the board. The legacy receive-file board/crooked-paper tail at frames 1216–1343 is intentionally not shipped.

Paper carry follows the new upper beak using a keyed Copy Location constraint. Slot placement uses a separate keyed reference and Copy Transforms influence. The mailbox door is constrained closed only during sending. File receipt suppresses the carried paper while retaining the mailbox note. Wing balance uses a small mirrored outward movement without changing the approved wing origins or modifiers.

The collision correction is authored in the same Blender file: the bird stops farther from the mailbox, the receiving door opens flat, and a sliding tray supports the incoming note. The paper and tray share their extension timing; the note hands off to the beak before the tray retracts. The bird holds the paper flat until the tray has withdrawn, then lifts its head and tips the paper downward. The carried note's last hidden pose already matches the receiving position, avoiding a visible jump when it appears. For delivery, the bird turns the paper flat outside the door before moving it through the slot. A Boolean aperture is applied to the ivory door mesh, and its hidden cutter must stay excluded from export. The board, carried and incoming notes use the same shortened paper proportions. Head contact poses and constraint influences remain editable in Blender.

The tray has a graphite material and rests on two rails connected to the inner side walls, so the white letter has visible support inside the mailbox. The rails remain fixed while the tray slides; sufficient overlap remains at full extension. `20 - Board clearance` is an animated parent of the bird root for the approach to the writing board. It separates standing distance from the head gesture, while the carried note still follows the native beak constraint. A small normal offset keeps the released sheet on the front surface of the board. During the receiving retreat, the clearance parent compensates for the legacy root offset at frames 1070 and 1078 with Blender location `(0.6, 0.25, 0)`, returning the bird to its idle position. File receipt now begins with that same offset at frame 1079, holds it through 1100, and eases to zero at 1116 after the bird has left the board.

## Runtime and review

The fixed orthographic camera has a 5° downward pitch, looking at `(0, 1.25, 0.1)`. The app supplies lights, theme treatment and a moving shadow. `Bird_Root` remains the stable hit region; `Nooboard_Form_09` drives the new bird's shadow. `composeMotionPreview.ts` remains available to test the earlier three-clip bird/legacy-prop composition but is not used by this complete-scene preview.

Runtime crossfades are limited to clips without carried props; business clip boundaries restore the exact authored prop state, avoiding a paper flying across the stage during blending. One-shot previews return to idle, paused holds its closed-eye pose, and offline loops. Reduced motion displays a representative still and releases the performance after 450 ms. Idle/offline do not animate in this mode. The light/dark `poster.jpg` and `poster-dark.jpg` fallbacks are 1200 × 700 captures of the same runtime scene; their hit regions match the rendered camera. The slider inspects a single authored pose with crossfade contributions cleared. Hidden pages stop rendering. All normal home controls continue using application state independently of preview playback.

Asset tests validate the clip set and finite tracks, carry attachment, file/mailbox semantics, door behavior, eye restoration, body clearance, tray support, handoff continuity, and the paper's passage through the slot and clearance during tray withdrawal. Slot and tray checks include quarter-frame samples between authored poses. Board checks inspect deformed body and paper vertices at half-frame intervals during both handoffs and the file-receipt departure; every clip must begin and end at the idle standing position. Rail checks verify contact with the tray, connection to the side walls, and overlap during extension. Integration tests exercise real snapshot decoding, file availability, fast and delayed receipts, mailbox grouping, wake/gesture priorities and hidden/reset behavior. Browser preview checks cover the default stage; two-device Tauri transfer validation remains separate from the scripted preview bridge.

Earlier `bird-motion-16.glb` and `bird-wing-18.glb` remain for comparison. Version 18's `18 - Soft oat wings` material uses sRGB `#D5CCBF`, roughness `0.9` and Principled IOR Level `0.18` (exported specular factor `0.36`). The current version retains that material.
