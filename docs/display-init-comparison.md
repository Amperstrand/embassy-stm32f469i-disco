# Display Init Comparison: Sync BSP vs Async BSP

## STM32F469 LTDC GCR Register Layout (from stm32-metapac)

Source: `stm32-data-generated/ab22d81/stm32-metapac/src/registers/ltdc_v1.rs`

```
GCR offset: 0x18 (from LTDC_BASE 0x4001_6800)

Bit  | Name   | Description
-----|--------|------------------------------------------
0    | LTDCEN | LCD-TFT controller enable
1    | -      | Reserved
4:6  | DBW    | Dither Blue Width (3 bits)
8:10 | DGW    | Dither Green Width (3 bits)
12:14| DRW    | Dither Red Width (3 bits)
16   | DEN    | Dither Enable (NOT Data Enable!)
28   | PCPOL  | Pixel Clock Polarity
29   | DEPOL  | Data Enable Polarity
30   | VSPOL  | Vertical Sync Polarity
31   | HSPOL  | Horizontal Sync Polarity
```

**CRITICAL BUG IN OUR CODE**: We set bit 1 thinking it's "DEN" (Data Enable), but bit 1 is RESERVED on STM32F469.
The actual DEN bit is at bit 16 (Dither Enable), and LTDCEN is at bit 0.
Embassy's PAC has no "den" alias at bit 1 — it only has `ltdcen` (bit 0), `dgw` (bits 8:10), and `den` (bit 16).

## LTDC Global Register Map (stm32-metapac, verified correct)

```
Offset  Register   Description
------  ---------  -----------
0x00   (IDR)      Identification Register (read-only)
0x08   SSCR       Synchronization Size Configuration
0x0C   BPCR       Back Porch Configuration
0x10   AWCR       Active Width Configuration
0x14   TWCR       Total Width Configuration
0x18   GCR        Global Control Register
0x24   SRCR       Shadow Reload Configuration
0x2C   BCCR       Background Color Configuration
0x34   IER        Interrupt Enable Register
0x40   ISR        Interrupt Status Register
0x44   ICR        Interrupt Clear Register
0x84   L1CR       Layer 1 Control Register
0x88   L1WHPCR    Layer 1 Window Horizontal Position
0x8C   L1WVPCR    Layer 1 Window Vertical Position
0x94   L1PFCR     Layer 1 Pixel Format
0x98   L1CACR     Layer 1 Constant Alpha
0x9C   L1DCCR     Layer 1 Default Color
0xA0   L1BFCR     Layer 1 Blending Factor
0xAC   L1CFBAR    Layer 1 Color Frame Buffer Address
0xB0   L1CFBLR    Layer 1 Color Frame Buffer Line Length
0xB4   L1CFBLNR   Layer 1 Color Frame Buffer Line Number
```

Note: SSCR is at 0x08 (NOT 0x00). There is a reserved/IDR register at 0x00.

## Sync BSP Init Order (WORKING — three sources compared)

### Source 1: stm32f469i-disc BSP `init_display_full()` (ea3b1b2)
### Source 2: stm32f4xx-hal amperstrand fork `new()` + `config_layer()` (05d999d)
### Source 3: embassy-stm32 F469 DSI BSP example `dsi_bsp.rs` (84444a19)

The sync init uses `DisplayController::new()` (NOT `new_dsi()`). Key characteristics:
- Uses PAC-generated register accessors (correct offsets, correct bit fields)
- LTDC initialized via `DisplayController::new()` which does PLLSAI + timing + GCR + SRCR
- Layer config done AFTER panel init via `config_layer()` + `enable_layer()` + `reload()`
- Does NOT use `dsi.refresh()` (no WCR.LTDCEN) — relies on GCR.LTDCEN instead
- Embassy's `Ltdc::setup_clocks()` calls `rcc::enable_and_reset::<LTDC>()` which does proper RCC enable+reset

### Detailed Sync Init Sequence

```
1. LCD Reset
   - PH7 low, delay 20ms, PH7 high, delay 10-140ms

2. DSI Host Init (DsiHost::init or equivalent)
   - DSI clock enable (RCC)
   - DSI EN=1
   - Regulator enable (WRPCR.REGEN=1), wait RRS
   - PLL config: NDIV=125, IDF=2, ODF=0, PLLEN=1, wait PLLLS
   - Clock/digital enable (PCTLR.CKE=1, DEN=1)
   - DPCC=1, ACR=0
   - 2 data lanes (PCONFR.NL=1)
   - TXECKDIV=4
   - UIX4=8 (embassy BSP) or 13 (our code) — differs
   - Disable interrupts (IER0=0, IER1=0)
   - BTAE=1 (PCR)
   - Video mode: MCR.CMDM=0, WCFGR.DSIM=0
   - VMCR: VMT=burst (0b10), LP transition enables, LPCE=1
   - VPCR=active_width
   - VCCR: NUMC=1
   - VNPCR=0
   - DSI timing (VHSACR, VHBPCR, VLCR, VVSACR, VVBPCR, VVFPCR, VVACR)
   - LP/VLP sizes (LPMCR)
   - PHY timers (CLTCR, DLTCR, PCONFR.SW_TIME)
   - Video mode: MCR.CMDM=0, WCFGR.DSIM=0

3. Switch to LP command mode for panel communication
   - CMCR: all GSxTX/DSxTX bits = is_low_power, ARE=0

4. DSI Start
   - CR.EN=1
   - WCR.DSIEN=1 (bit 3!)

5. Enable bus turnaround (for reads)
   - PCONFR.TAS=1 or similar

6. Panel detection (probe NT35510 via DSI reads)

7. LTDC Init (DisplayController::new or equivalent)
   a. RCC: enable LTDC clock, reset LTDC peripheral
   b. PLLSAI config: write PLLSAICFGR (N, R only — zeros Q!), set PLLSAIDIVR, enable PLLSAI, wait RDY
   c. LTDC timing: SSCR, BPCR, AWCR, TWCR
   d. GCR: HSPOL, VSPOL, DEPOL, PCPOL polarity bits
   e. BCCR: background color
   f. SRCR reload (IMR=1)
   g. GCR: LTDCEN=1 (bit 0)
      NOTE: sync HAL does NOT set bit 1 or bit 16 for "DEN"
      Embassy's GCR has NO bit 1 field. Bit 16 is DEN (Dither Enable).
   h. SRCR reload (IMR=1)

8. Switch to LP command mode again
   - Same CMCR as step 3

9. Force RX low power
   - WPCR1.FLPRXLPM=1

10. Panel init (NT35510 init_rgb565 or equivalent DCS commands)
    - SETEXTC, B0-BF power registers
    - TEEON, COLMOD, SLPOUT, MADCTL, CASET, RASET
    - WRDISBV, WRCTRLD, WRCABC, WRCABCMB
    - DISPON, RAMWR

11. Force RX low power off
    - WPCR1.FLPRXLPM=0

12. Switch to HS command mode
    - CMCR: all GSxTX/DSxTX bits = false, ARE=0

13. Layer config (config_layer)
    - L1WHPCR, L1WVPCR (window position)
    - L1PFCR (pixel format: RGB565 = 0x02)
    - L1CACR (constant alpha = 255)
    - L1DCCR (default color: opaque red 0xFFFF0000)
    - L1BFCR: bf1=CONSTANT, bf2=CONSTANT
    - L1CFBAR (framebuffer address)
    - L1CFBLR (line length + 3, pitch)
    - L1CFBLNR (line count)

14. Enable layer (enable_layer)
    - L1CR.LEN=1

15. Reload (reload)
    - SRCR.IMR=1

16. DSI wrapper enable (if not already)
    - WCR.DSIEN=1 (already done in step 4)
    - NOTE: NO WCR.LTDCEN! The sync path does NOT set this bit.
      The embassy F469 BSP example does NOT call dsi.refresh() or set WCR.LTDCEN.
```

## Async BSP Init Order (BROKEN — fractal noise)

Source: `embassy-stm32f469i-disco/src/display.rs`

```
1. LCD Reset — same as sync ✓

2. dsi_init() — raw register writes, same logical sequence as sync ✓
   - Correct PLL, timing, video mode config
   - BUT: UIX4=13 (sync uses 8 in embassy example, sync HAL calculates from freq)

3. ltdc_init() — raw register writes
   a. PLLSAIDIVR touch (bits 17:16 of DCKCFGR) — correct
   b. APB2RSTR LTDCRST + DMA2DRST — manual reset (not rcc::enable_and_reset)
      PROBLEM: may not properly enable LTDC clock before reset
   c. GCR = 0xB0000000 (bits 28, 30, 31) — sets PCPOL, DEPOL, HSPOL
      BUG: bits 28=PCPOL ✓, 30=DEPOL ✓, 31=HSPOL ✓ but this also clears all other bits
   d. SSCR, BPCR, AWCR, TWCR — correct ✓
   e. BCCR = 0 (sync uses 0xAAAAAAAA)
   f. IER = (1<<2)|(1<<1)
   g. Layer config (L1WHPCR through L1CR.LEN=1) — WRONG POSITION (should be after panel init)
   h. SRCR reload
   i. GCR |= (1<<0)|(1<<1) — BUG: bit 1 is RESERVED, not DEN!
   j. SRCR reload

4. detect_panel() — instant return for ForceNt35510 ✓

5. dsi_set_lp_command_mode() — CMCR LP bits ✓

6. NT35510 init_rgb565() — same DCS commands ✓

7. dsi_set_hs_command_mode() — CMCR HS bits ✓
   reg32_set(DSI_BASE, 0x404, 1 << 2) — WCR.LTDCEN=1
   BUG: sync does NOT set WCR.LTDCEN!

8. ltdc_config_layer() — moved here after commit 0b85a0f ✓
```

## Bug Summary

### Bug 1: GCR bit 1 is RESERVED (CRITICAL)
We write `(1 << 0) | (1 << 1)` to GCR thinking bit 1 is "DEN". But on STM32F469:
- Bit 0 = LTDCEN (correct)
- Bit 1 = RESERVED
- Bit 16 = DEN (Dither Enable)

The sync HAL uses PAC-generated accessors: `ltdc.gcr().modify(|_, w| w.ltdcen().set_bit().den().set_bit())`.
In the PAC, `den()` maps to bit 16 (Dither Enable), NOT bit 1.

However, the sync BSP `new_dsi()` at line 468 does:
```rust
ltdc.gcr().modify(|_, w| w.ltdcen().set_bit().den().set_bit());
```
This sets bit 0 (LTDCEN) and bit 16 (DEN = Dither Enable).

But the embassy F469 BSP example at line 365 does:
```rust
ltdc.enable(); // only sets ltdcen
```

So the working example ONLY sets LTDCEN (bit 0). It does NOT set DEN at all.

### Bug 2: WCR.LTDCEN (WRONG — sync does NOT use this)
We set `WCR.LTDCEN=1` (DSI wrapper bit 2). The sync BSP does NOT do this.
The embassy F469 BSP example does NOT call `dsi.refresh()`.
GCR.LTDCEN (bit 0) is sufficient to enable the LTDC pixel stream.

### Bug 3: GCR initial write 0xB0000000 clears all other GCR bits
We write `0xB0000000` (bits 28, 30, 31) which clears all other GCR bits.
This is OK as a write, but then the SRCR reload happens while GCR has no polarity bits set.
The sync HAL writes polarity bits first, THEN reloads, THEN enables LTDCEN.

### Bug 4: PLLSAIDIVR mismatch
- Our code: PLLSAIDIVR = 0b00 (no division) → pixel clock = PLLSAI_R / 1
- Embassy BSP: PLLSAIDIVR = DIV2 → pixel clock = PLLSAI_R / 2
- Sync HAL: calculates dynamically, likely DIV2

With PLLSAI_R=7 and PLLSAI_N=384:
- No division: 168MHz * 384 / 8 / 7 = 27.43 MHz
- DIV2: 168MHz * 384 / 8 / 7 / 2 = 13.71 MHz

The embassy BSP uses PLLSAIDIVR=DIV2, which halves the pixel clock. This might affect DSI timing.

### Bug 5: PLLSAI Q zeroed by sync HAL
The sync HAL's `new_dsi()` writes `rcc.pllsaicfgr().write()` with only N and R, which zeros Q.
This would break USB (which needs PLLSAI_Q=48MHz).
Our async BSP preserves embassy's PLLSAI config (including Q), which is correct for USB.

### Bug 6: L1BFCR BF2 (FIXED in commit 0b85a0f)
Was 0x0405 (pixel alpha), fixed to 0x0407 (constant alpha). Correct fix.

### Bug 7: Layer config order (FIXED in commit 0b85a0f)
Was inside ltdc_init(), moved after panel init + WCR.LTDCEN. Correct fix.

## What the RTT dump showed (commit 0b85a0f)

```
LTDC GCR  = 00010200 (LTDCEN=0, DEN=0)
DSI  WCR  = 00000008 (DSIEN=1, LTDCEN=0)
```

GCR reads back 0x00010200:
- Bit 9 = 1 (DGW bit 1 = 1, DGW = 010 = 2)
- Bit 16 = 1 (DEN = Dither Enable = 1)

This is NOT what we wrote (0xB0000003). Something is overwriting GCR after our write.
Most likely: our SRCR reload (step h in ltdc_init) triggers a shadow register reload
that resets GCR to some default because the LTDC was not properly enabled.

Or: the manual APB2RSTR reset in step b doesn't work correctly because we didn't
properly enable the LTDC clock via RCC before resetting.

## Corrected Async Init Sequence

Based on the embassy F469 BSP example (the most directly applicable reference):

```
1. LCD Reset
2. Ltdc::new() — embassy's setup_clocks() does rcc::enable_and_reset::<LTDC>() + PLLSAIDIVR=DIV2
3. DSI init (same as current)
4. LTDC disable (GCR.LTDCEN=0)
5. LTDC timing: SSCR, BPCR, AWCR, TWCR
6. LTDC GCR: polarity bits only (HSPOL, VSPOL, DEPOL, PCPOL)
7. LTDC BCCR: background color
8. LTDC IER: interrupts
9. LTDC enable (GCR.LTDCEN=1) — ONLY set bit 0, not bit 1 or bit 16
10. DSI enable + wrapper enable
11. Panel init (NT35510 DCS commands)
12. Layer config (L1WHPCR through L1CR.LEN=1)
13. SRCR reload
```

Key differences from current code:
- Use embassy's `rcc::enable_and_reset` instead of manual APB2RSTR
- Set PLLSAIDIVR to DIV2 (not no-division)
- GCR: set polarity, then ONLY set LTDCEN (bit 0), no other bits
- Remove WCR.LTDCEN entirely
- Layer config AFTER panel init, with final SRCR reload
