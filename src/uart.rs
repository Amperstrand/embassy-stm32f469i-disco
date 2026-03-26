use embassy_stm32::usart::{Config, Uart};

pub const DEFAULT_BAUDRATE: u32 = 115200;

pub struct UartCtrl<'d> {
    pub inner: Uart<'d, embassy_stm32::mode::Blocking>,
}

impl<'d> UartCtrl<'d> {
    pub fn new_usart6(
        p_usart6: embassy_stm32::Peri<'d, embassy_stm32::peripherals::USART6>,
        p_rx: embassy_stm32::Peri<'d, embassy_stm32::peripherals::PG9>,
        p_tx: embassy_stm32::Peri<'d, embassy_stm32::peripherals::PG14>,
        baudrate: u32,
    ) -> Self {
        let mut uart_config = Config::default();
        uart_config.baudrate = baudrate;
        let inner = Uart::new_blocking(p_usart6, p_rx, p_tx, uart_config).unwrap();
        Self { inner }
    }

    pub fn read_byte(&mut self) -> nb::Result<u8, embassy_stm32::usart::Error> {
        let mut buf = [0u8; 1];
        self.inner.blocking_read(&mut buf).map_err(|e| nb::Error::Other(e))?;
        Ok(buf[0])
    }

    pub fn write_byte(&mut self, byte: u8) -> nb::Result<(), embassy_stm32::usart::Error> {
        self.inner.blocking_write(&[byte]).map_err(|e| nb::Error::Other(e))
    }

    pub fn flush(&mut self) -> nb::Result<(), embassy_stm32::usart::Error> {
        self.inner.blocking_flush().map_err(|e| nb::Error::Other(e))
    }
}
