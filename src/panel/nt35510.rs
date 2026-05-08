use embassy_stm32::{dsihost, peripherals};
use embassy_time::{block_for, Duration};
use embedded_display_controller::dsi::DsiHostCtrlIo;
use embedded_hal::delay::DelayNs;
use nt35510::Nt35510;
#[cfg(feature = "display")]
use otm8009a::Otm8009A;

use crate::display::{DisplayFormat, DisplayInitError, DisplayOrientation, FB_HEIGHT, FB_WIDTH};
use crate::dsi::DsiHostAdapter;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

/// Adapter for OTM8009A (requires embedded-hal 0.2 blocking delay traits).
#[cfg(feature = "display")]
struct DelayMsAdapter;

#[cfg(feature = "display")]
impl embedded_hal_02::blocking::delay::DelayMs<u32> for DelayMsAdapter {
    fn delay_ms(&mut self, ms: u32) {
        block_for(Duration::from_millis(ms as u64));
    }
}

/// Supported LCD panel controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdController {
    /// NT35510 panel (default on STM32F469I-Discovery).
    Nt35510,
    /// OTM8009A panel (alternate).
    Otm8009a,
}

/// Hint for panel auto-detection during [`DisplayCtrl::new()`](crate::DisplayCtrl::new).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardHint {
    /// Probe the panel via DSI reads and fall back to NT35510 on failure.
    Auto,
    /// Skip probe, assume NT35510. Recommended when DSI reads are unreliable.
    ForceNt35510,
    /// Skip probe, assume OTM8009A.
    ForceOtm8009a,
}

/// Detect the LCD panel controller via DSI command-mode reads.
///
/// When [`BoardHint::Auto`] is given, attempts up to 3 NT35510 probes via DSI.
/// Falls back to [`LcdController::Nt35510`] if all probes fail (DSI reads are
/// unreliable on this board — see Known Issues in AGENTS.md).
/// Forced hints ([`BoardHint::ForceNt35510`], [`BoardHint::ForceOtm8009a`])
/// skip the probe entirely.
pub fn detect_panel(dsi: &mut impl DsiHostCtrlIo, hint: BoardHint) -> LcdController {
    if hint == BoardHint::ForceNt35510 {
        return LcdController::Nt35510;
    }
    if hint == BoardHint::ForceOtm8009a {
        return LcdController::Otm8009a;
    }

    let mut panel = Nt35510::new();
    let mut mismatch_count = 0u32;
    let mut first_mismatch_id: u8 = 0;

    for _attempt in 0..3 {
        match panel.probe(dsi) {
            Ok(()) => {
                #[cfg(feature = "defmt")]
                defmt::info!("detect_panel: NT35510 detected on attempt {}", _attempt + 1);
                return LcdController::Nt35510;
            }
            Err(nt35510::Error::ProbeMismatch(id)) => {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "detect_panel: attempt {} mismatch, RDID2=0x{:02x}",
                    _attempt + 1,
                    id
                );
                if mismatch_count == 0 {
                    first_mismatch_id = id;
                }
                mismatch_count += 1;
            }
            Err(nt35510::Error::DsiRead) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} DSI read error", _attempt + 1);
            }
            Err(_) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} unknown error", _attempt + 1);
            }
        }
        BusyDelay.delay_ms(5);
    }

    #[cfg(feature = "defmt")]
    defmt::info!(
        "detect_panel: {} mismatches, first_id=0x{:02x} — falling back to NT35510",
        mismatch_count,
        first_mismatch_id
    );

    #[cfg(feature = "display")]
    if mismatch_count >= 2 && first_mismatch_id != 0 {
        let mut otm = Otm8009A::new();
        if otm.id_matches(dsi).unwrap_or(false) {
            return LcdController::Otm8009a;
        }
    }

    LcdController::Nt35510
}

pub(crate) fn init_panel<'d, F: DisplayFormat>(
    dsi: &mut dsihost::DsiHost<'d, peripherals::DSIHOST>,
    controller: LcdController,
    orientation: DisplayOrientation,
) -> Result<(), DisplayInitError> {
    match controller {
        LcdController::Nt35510 => {
            BusyDelay.delay_ms(120);
            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: starting NT35510 init via nt35510 crate");

            let mut panel = Nt35510::new();
            let mut dsi_adapter = DsiHostAdapter::new(dsi);
            let mut delay = BusyDelay;
            let config = nt35510::Nt35510Config {
                mode: orientation.nt35510_mode(),
                color_map: nt35510::ColorMap::Rgb,
                color_format: F::nt35510_color_format(),
                cols: FB_WIDTH,
                rows: FB_HEIGHT,
            };
            panel
                .init_with_config(&mut dsi_adapter, &mut delay, config)
                .map_err(|_| DisplayInitError::PanelInit)?;

            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: NT35510 init done");
        }
        #[cfg(feature = "display")]
        LcdController::Otm8009a => {
            use otm8009a::{ColorMap, FrameRate, Mode, Otm8009AConfig};

            BusyDelay.delay_ms(120);
            let mut panel = Otm8009A::new();
            let mut dsi_adapter = DsiHostAdapter::new(dsi);
            let mut delay = DelayMsAdapter;
            let config = Otm8009AConfig {
                frame_rate: FrameRate::_60Hz,
                mode: Mode::Landscape,
                color_map: ColorMap::Rgb,
                cols: FB_WIDTH,
                rows: FB_HEIGHT,
            };
            panel
                .init(&mut dsi_adapter, config, &mut delay)
                .map_err(|_| DisplayInitError::PanelInit)?;
        }
    }

    Ok(())
}
