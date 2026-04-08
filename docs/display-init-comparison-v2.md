# Sync vs Async Display Init Comparison (v2)

**Date**: 2026-04-08  
**BSP commit**: f54c992  
**Status**: Display shows consistent noise. LTDC output NOT reaching panel.

## Hardware Verification Result

- DIAG 0b (layer disabled, BCCR=RED): Noise — NOT solid red
- TEST 1 (black→white→black): No change in display
- TEST 2-6 (checkerboard, color bars, stripes, block, blink): No change
- **Conclusion**: DSI video stream is NOT carrying LTDC pixel data. The panel shows
  its own GRAM content (consistent power-on pattern), not LTDC framebuffer.

---

## Init Ordering Comparison

### Sync BSP (`init_display_full` in lcd.rs)

```
1. DSI RCC enable (APB2ENR bit 27)
2. DSI CR.EN = 1 (early, before PLL config)
3. DSI WRPCR REGEN = 1, wait RRS
4. DSI PLL config (NDIV=125, IDF=2, ODF=0)
5. DSI WRPCR PLLEN = 1, wait PLLLS
6. DSI PCTLR CKE=1, DEN=1
7. DSI CLCR DPCC=1, ACR=0
8. DSI PCONFR NL=1 (2 lanes)
9. DSI CCR TXECKDIV=4
10. DSI WPCR0 UIX4=13
11. DSI IER0=0, IER1=0
12. DSI MCR CMDM=0 (video mode)
13. DSI WCFGR DSIM=0, TESRC=0, TEPOL=0, AR=0
14. DSI VMCR: VMT=2(burst), LPVSAE/LPVBPE/LPVFPE/LPVAE/LPHBPE/LPHFPE/LPCE=1, FBTAAE=0
15. DSI VPCR VPSIZE=480
16. DSI VCCR NUMC=1
17. DSI VNPCR NPSIZE=0
18. DSI VHSACR/VHBPCR/VLCR/VVSACR/VVBPCR/VVFPCR/VVACR (timing)
19. DSI LVCIDR VCID=0
20. DSI LPCR DEP=0, VSP=0, HSP=0
21. DSI LCOLCR COLC=0 (RGB565), LPE=0
22. DSI WCFGR COLMUX=0 (RGB565)
23. DSI LPMCR LPSIZE=64, VLPSIZE=64
24. DSI CMCR: All LP bits=1, ARE=0
25. DSI CR.EN=1 (again, in start())
26. DSI WCR DSIEN=1 (in start())
27. DSI PCR BTAE=1
28. 20ms delay
29. Detect LCD controller
30. LTDC RCC enable (APB2ENR bit 26)
31. LTDC RCC reset (APB2RSTR bit 26 set/clear)
32. DMA2D RCC enable (AHB1ENR bit 23)
33. DMA2D RCC reset (AHB1RSTR bit 23 set/clear)
34. RCC PLLSAICFGR (PLLSAIN, PLLSAIR)
35. RCC DCKCFGR PLLSAIDIVR
36. RCC CR PLLSAION=1, wait PLLSAIRDY
37. LTDC SSCR, BPCR, AWCR, TWCR (timing)
38. LTDC GCR: HSPOL=1, VSPOL=1, DEPOL=0, PCPOL=1
39. LTDC BCCR = 0xAAAAAAAA
40. LTDC SRCR IMR=1 (reload)
41. LTDC GCR LTDCEN=1, DEN=1
42. LTDC SRCR IMR=1 (reload)
43. DSI CMCR All LP bits=1 (panel init mode)
44. DSI WPCR1 force_rx_low_power=true
45. NT35510 init_rgb565() — panel commands
46. DSI WPCR1 force_rx_low_power=false
47. DSI CMCR All LP bits=0 (HS mode)
48. config_layer, enable_layer, reload
```

### Async BSP (`DisplayCtrl::new` in display.rs)

```
1. DSI CR CMDM=0, WCFGR DSIM=0, CR EN=0 (shutdown)
2. DSI PCTLR CKE=0, DEN=0
3. DSI WRPCR PLLEN=0, REGEN=0
4. Enable APB2 clocks (APB2ENR bits 26,27)
5. DSI WRPCR REGEN=1, wait RRS
6. DSI PLL config (NDIV=125, IDF=2, ODF=0)
7. DSI WRPCR PLLEN=1, wait PLLLS
8. DSI PCTLR CKE=1, DEN=1
9. DSI CLCR DPCC=1
10. DSI PCONFR NL=1 (2 lanes)
11. DSI CCR TXECKDIV=4
12. DSI WPCR0 UIX4=13
13. DSI IER0=0, IER1=0
14. DSI PCR BTAE=1
15. DSI CR CMDM=0 (video mode, again)
16. DSI WCFGR DSIM=0, TESRC=0, TEPOL=0, AR=0
17. DSI VPCR VPSIZE=480
18. DSI VCCR NUMC=1
19. DSI VNPCR NPSIZE=0
20. DSI LVCIDR VCID=0
21. DSI LPCR = 0
22. DSI LCOLCR COLC=0 (RGB565)
23. DSI WCFGR COLMUX=0 (RGB565)
24. DSI VMCR: VMT=2(burst), all LP bits set
25. DSI VHSACR/VHBPCR/VLCR/VVSACR/VVBPCR/VVFPCR/VVACR (timing)
26. DSI LPMCR LPSIZE=64, VLPSIZE=64
27. DSI CLTCR, DLTCR (PHY timers)
28. DSI PCONFR SW_TIME=10
29. DSI CR EN=1 (at end of dsi_init)
30. DSI WCR DSIEN=1 (at end of dsi_init)
31. LTDC init: verify PLLSAIRDY
32. RCC DCKCFGR PLLSAIDIVR=DIV2
33. LTDC+DMA2D reset (APB2RSTR bit 26, AHB1RSTR bit 23)
34. LTDC GCR: HSPOL=1, VSPOL=1, PCPOL=1 (no DEN)
35. LTDC SSCR, BPCR, AWCR, TWCR (timing)
36. LTDC BCCR=0
37. LTDC IER: TERRIE, FUIE
38. LTDC SRCR reload
39. LTDC GCR LTDCEN=1
40. LTDC SRCR reload
41. detect_panel → ForceNt35510
42. dsi_set_lp_command_mode()
43. NT35510 init_rgb565()
44. dsi_set_hs_command_mode()
45. ltdc_config_layer()
```

### Embassy Working Example (`dsi_bsp.rs`)

```
1. LCD reset (PH7)
2. Ltdc::new() → setup_clocks (PLLSAIDIVR=DIV2), enable_and_reset LTDC
3. DsiHost::new() → enable_and_reset DSI
4. DSI shutdown (disable wrapper, disable host, disable PHY, PLL, regulator)
5. DSI regulator enable, wait
6. DSI PLL config (NDIV=125, IDF=2, ODF=0), wait lock
7. DSI PHY enable (CKE=1, DEN=1)
8. DSI CLCR DPCC=1
9. DSI PCONFR NL=1
10. DSI CCR TXECKDIV=4
11. DSI WPCR0 UIX4=8
12. DSI IER0=0, IER1=0
13. DSI PCR BTAE=1
14. DSI MCR CMDM=0, WCFGR DSIM=0 (video mode)
15. DSI VMCR: VMT=2(burst), LPCE=1, all LP bits
16. DSI VPCR VPSIZE=800
17. DSI VCCR NUMC=0
18. DSI VNPCR NPSIZE=0xFFF
19. DSI LVCIDR VCID=0
20. DSI LPCR DEP=0, HSP=0, VSP=0
21. DSI LCOLCR COLC=5 (RGB888)
22. DSI WCFGR COLMUX=5 (RGB888)
23. DSI timing registers (LANDSCAPE 800x480)
24. DSI VMCR LP transition enables
25. DSI CLTCR, DLTCR, PCONFR (timers)
26. LTDC GCR disable (explicit)
27. LTDC timing (LANDSCAPE 800x480)
28. LTDC BCCR = 0 (black)
29. LTDC IER: TERRIE, FUIE
30. LTDC enable (GCR LTDCEN=1)
31. DSI enable (CR EN=1)
32. DSI enable_wrapper_dsi (WCR DSIEN=1)
33. 120ms delay
34. NT35510 init commands (RGB888, landscape)
35. NT35510 RAMWR (0x2C)
36. LTDC layer config (ARGB8888)
```

---

## Critical Differences

### 1. Pixel Format End-to-End

| Component | Sync BSP | Embassy Example | Async BSP |
|-----------|-----------|----------------|------------|
| Panel COLMOD | RGB565 (0x04) | RGB888 (0x77) | RGB565 (0x04) |
| DSI LCOLCR | 0 (RGB565) | 5 (RGB888) | 0 (RGB565) |
| DSI WCFGR COLMUX | 0 (RGB565) | 5 (RGB888) | 0 (RGB565) |
| LTDC L1PFCR | RGB565 | ARGB8888 | RGB565 |

Sync uses RGB565 everywhere. Embassy uses RGB888 everywhere. Async matches sync.
**Not the issue** — sync works with RGB565.

### 2. WCR.LTDCEN (DSI Wrapper LTDC Enable)

| Implementation | Sets WCR.LTDCEN? |
|---------------|-------------------|
| Sync BSP `start()` | No (only sets DSIEN) |
| Sync BSP `refresh()` | Yes (but NOT called in init flow) |
| Embassy example | No (never sets it) |
| Async BSP | No |

**Not the issue** — no working implementation sets this bit.

### 3. LTDC GCR.DEN (Dither Enable)

| Implementation | GCR.DEN? |
|---------------|-----------|
| Sync BSP | Yes (bit 1, set with LTDCEN) |
| Embassy example | No |
| Async BSP | No (removed in f54c992) |

**Probably not the issue** — embassy works without it.

### 4. Blending Factors (L1BFCR)

| Implementation | BF1 | BF2 | Raw Value |
|---------------|-----|-----|-----------|
| Sync BSP | CONSTANT_ALPHA (4) | CONSTANT_ALPHA (5) | 0x0405 |
| Embassy example | CONSTANT (4) | CONSTANT (4) | 0x0404 |
| Async BSP | CONSTANT (4) | CONSTANT (7) | 0x0407 |

BF2 value 7 vs 5 — both are "constant alpha" variants. Different encoding.
**Possibly relevant** but unlikely to cause complete data disconnection.

### 5. WPCR0 UIX4

| Implementation | UIX4 | Source |
|---------------|-------|--------|
| Sync BSP | 13 | Calculated from f_phy=312.5MHz |
| Embassy example | 8 | Hardcoded |
| Async BSP | 13 | Calculated (matches sync) |

**Probably not the issue** — sync works with 13.

### 6. VCCR NUMC (Video Chunk Count)

| Implementation | NUMC |
|---------------|------|
| Sync BSP | 1 |
| Embassy example | 0 |
| Async BSP | 1 |

**Probably not the issue** — both 0 and 1 are valid.

### 7. DSI LPCR (LTDC Polarity Configuration)

| Implementation | DEP | HSP | VSP |
|---------------|-----|-----|-----|
| Sync BSP | 0 | 0 | 0 |
| Embassy example | 0 | 0 | 0 |
| Async BSP | 0 | 0 | 0 |

Same. **Not the issue.**

### 8. Background Color (BCCR)

| Implementation | Value |
|---------------|-------|
| Sync BSP | 0xAAAAAAAA (gray) |
| Embassy example | 0x00000000 (black) |
| Async BSP | 0x00000000 (black) |

**Not the issue.**

### 9. Default Layer Color (L1DCCR)

| Implementation | Value |
|---------------|-------|
| Sync BSP | 0xFFFF0000 (opaque red) |
| Embassy example | 0x00000000 (transparent black) |
| Async BSP | 0x00000000 (transparent black) |

**Not the issue** — embassy works with transparent black.

### 10. CFBLL (Frame Buffer Line Length)

| Implementation | CFBLL Calculation |
|---------------|------------------|
| Sync BSP | width * bpp + 3 |
| Embassy example | width * bpp + 7 (for F4) |
| Async BSP | width * bpp + 7 (changed in f54c992) |

Both +3 and +7 are valid per STM32 errata. Sync uses +3, embassy uses +7.
**Possibly relevant** — try changing back to +3.

### 11. NT35510 TEEON Ordering

| Implementation | TEEON timing |
|---------------|--------------|
| Sync BSP (fork) | Before SLOPOUT, after COLMOD |
| Embassy example | After SLOPOUT, after MADCTL |
| Async BSP (fork) | Before SLOPOUT, after COLMOD |

Same (fork uses same nt35510). **Not the issue.**

### 12. Init Ordering

| Step | Sync BSP | Embassy | Async BSP |
|------|-----------|---------|-----------|
| DSI enable (CR.EN) | Before config | After config | After config |
| LTDC enable (GCR.LTDCEN) | After DSI | After DSI | After DSI |
| Panel init | After LTDC | After DSI+LTDC | After DSI+LTDC |
| Layer config | After panel init | After panel init | After panel init |

All three have DSI and LTDC enabled before panel init.
**Not the issue.**

---

## What's LEFT to investigate

All register values match. All ordering is similar. The display still shows noise.
The noise is consistent across boots (not random SDRAM content).

### Hypothesis: The DSI video stream is being sent but carries no valid data

If DSI is in video mode but LTDC is not actually driving pixel data, the DSI link
would transmit... what? The DSI host in video mode gets pixels from the LTDC via an
internal bus. If LTDC is enabled but not actually fetching from SDRAM (due to some
uninitialized state), the DSI would send garbage or default values.

### Key diagnostic needed: DSI WISR error flags during display

Check if DSI reports any errors (TE errors, PSE errors, etc.) during the noise display.
The embassy example enables LTDC IER TERRIE and FUIE — check if those fire.

### Recommended next step: Phase 2 — Replace raw LTDC code with embassy Ltdc driver

Replace `ltdc_init()` + `ltdc_config_layer()` with embassy's `Ltdc::new()` +
`ltdc.init()` + `ltdc.init_layer()`. This eliminates ~100 lines of raw register code
and uses tested PAC-generated driver code that is verified working on the same hardware.

The embassy driver handles:
- Proper `rcc::enable_and_reset::<LTDC>()` sequence
- Correct PLLSAIDIVR configuration
- SRCR reload timing
- Layer configuration with verified CFBLL/CFBP calculations
- Error interrupt setup (TERRIE, FUIE)
- Immediate reload sequencing

This is the Prometheus plan Phase 2. Session ID: `ses_292f660cdffen2JNTNvEWXHvnD`
