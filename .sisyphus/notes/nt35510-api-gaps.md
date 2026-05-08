# T16 handoff: nt35510 API gaps surfaced by panel split

## Should live in upstream `nt35510` crate

- High-level `init_with_config()` coverage for the full STM32F469I-DISCO NT35510 bring-up sequence, so BSPs do not need to own controller-specific register programming details.
- Public configuration surface for color format / pixel packing choices already expressed by `nt35510::ColorFormat`, including any companion register writes needed to keep the panel sequence coherent.
- Public orientation/mode helpers tied to NT35510 panel init, since the BSP currently only supplies board wiring and chooses portrait vs landscape.
- A documented probe/init contract describing which DSI read/write failures are controller-level vs transport-level, so BSPs can implement fallback policy without duplicating controller knowledge.
- If additional NT35510 command tables are still hidden or crate-internal upstream, expose them through stable APIs rather than forcing BSP-local copies.

## Must stay in this BSP

- STM32 DSI host adapter glue (`DsiHostAdapter`) and any embassy-stm32-specific transport implementation.
- Board-specific reset timing around the LCD reset GPIO.
- Board policy enums and fallback behavior (`BoardHint`, `detect_panel`) because they encode this board's mixed NT35510/OTM8009A probe strategy and documented DSI read quirks.
- LTDC/DSI host orchestration and framebuffer sizing (`FB_WIDTH`, `FB_HEIGHT`) because they are board/display-stack integration concerns, not controller-generic logic.
- OTM8009A coexistence logic; that belongs in the BSP layer that chooses between multiple possible panels.
