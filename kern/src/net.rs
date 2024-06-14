///! smoltcp 抽象化でUSPiをラップするネットワークデバイス
pub mod uspi;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::fmt;
//use core::net::Ipv4Addr;
use core::time::Duration;
use aarch64::affinity;

use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache};
use smoltcp::phy::{self, Device, DeviceCapabilities};
use smoltcp::socket::{SocketHandle, SocketRef, TcpSocketBuffer};
use smoltcp::time::Instant;
use smoltcp::wire::{IpAddress, IpCidr};

use crate::mutex::Mutex;
use crate::param::MTU;
use crate::USB;
use crate::percore::get_preemptive_counter;

// 内部ストレージとして常に独自のバッファを使用する
pub type SocketSet = smoltcp::socket::SocketSet<'static, 'static, 'static>;
pub type TcpSocket = smoltcp::socket::TcpSocket<'static>;
pub type EthernetInterface<T> = smoltcp::iface::EthernetInterface<'static, 'static, 'static, T>;

/// 8-バイトアラインの `u8` スライス.
#[repr(align(8))]
struct FrameBuf([u8; MTU as usize]);

/// 長さ追跡ができる機能を持つ固定サイズのバッファ.
pub struct Frame {
    buf: Box<FrameBuf>,
    len: u32,
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("buf", &{ self.buf.as_ref() as *const FrameBuf })
            .field("len", &self.len)
            .finish()
    }
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            buf: Box::new(FrameBuf([0; MTU as usize])),
            len: MTU,
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.buf.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buf.0.as_mut_ptr()
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn set_len(&mut self, len: u32) {
        assert!(len <= MTU as u32);
        self.len = len;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf.0[..self.len as usize]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf.0[..self.len as usize]
    }
}

#[derive(Debug)]
pub struct UsbEthernet;

impl<'a> Device<'a> for UsbEthernet {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capability = DeviceCapabilities::default();
        capability.max_transmission_unit = MTU as usize;
        capability
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut frame = Frame::new();
        match USB.recv_frame(&mut frame) {
            Some(_) => {
                let rx = RxToken { frame };
                let tx = TxToken;
                Some((rx, tx))
            }
            _ => None,
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken)
    }
}

pub struct RxToken {
    frame: Frame,
}

impl phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(self.frame.as_mut_slice())
    }
}

pub struct TxToken;

impl phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut frame = Frame::new();
        frame.set_len(len.try_into().unwrap());
        let result = f(frame.as_mut_slice());
        USB.send_frame(&frame);
        result
    }
}

/// `UsbEthernet`構造体を使って新規ethernetインタフェースを作成して返す.
pub fn create_interface() -> EthernetInterface<UsbEthernet> {
    // Lab 5 2.B
    let device = UsbEthernet;
    let hw_addr = USB.get_eth_addr();
    let neighbor_cache = NeighborCache::new(BTreeMap::new());
    let ip_addrs = [IpCidr::new(IpAddress::v4(192, 168, 10, 110), 24),
                    IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8)];

    EthernetInterfaceBuilder::new(device)
        .ethernet_addr(hw_addr)
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .finalize()
}

const PORT_MAP_SIZE: usize = 65536 / 64;

pub struct EthernetDriver {
    /// ソケットセット
    socket_set: SocketSet,
    /// ポートの利用状況を追跡するビットマップ
    port_map: [u64; PORT_MAP_SIZE],
    /// 内部ethernetインタフェース
    ethernet: EthernetInterface<UsbEthernet>,
}

impl EthernetDriver {
    /// 新規ethernetドライバを作成する.
    fn new() -> EthernetDriver {
        // Lab 5 2.B
        EthernetDriver {
            socket_set: SocketSet::new(vec![]),
            port_map: [0_u64; PORT_MAP_SIZE],
            ethernet: create_interface(),
        }
    }

    /// ethernetインタフェースをポーリングする.
    /// `smoltcp::iface::EthernetInterface::poll()`も参照のこと.
    fn poll(&mut self, timestamp: Instant) {
        // Lab 5 2.B
        match self.ethernet.poll(&mut self.socket_set, timestamp) {
            Ok(_) => {}
            Err(_) => {}
        }
    }

    /// 次回 `poll()` を呼び出すまでの待機時間を返す.
    /// `smoltcp::iface::EthernetInterface::poll_delay()`も参照のこと.
    fn poll_delay(&mut self, timestamp: Instant) -> Duration {
        // Lab 5 2.B
        match self.ethernet.poll_delay(&mut self.socket_set, timestamp) {
            Some(duration) => Duration::from_millis(duration.millis),
            None => Duration::from_millis(0),
        }
    }

    /// ポートを使用済みとマークする。成功したら`Some(port)`を返し、
    /// 失敗したら`None`を返す。
    pub fn mark_port(&mut self, port: u16) -> Option<u16> {
        // Lab 5 2.B
        let idx = port as usize / PORT_MAP_SIZE;
        let map: u64 = 1_u64 << (port as usize % PORT_MAP_SIZE);
        if self.port_map[idx] & map != 0 {
            None
        } else {
            self.port_map[idx] |= map;
            Some(port)
        }
    }

    /// ポートの使用済みビットをクリアする。成功したら`Some(port)`を返し、
    /// 失敗したら`None`を返す。
    pub fn erase_port(&mut self, port: u16) -> Option<u16> {
        // Lab 5 2.B
        let idx = port as usize / PORT_MAP_SIZE;
        let map: u64 = 1_u64 << (port as usize % PORT_MAP_SIZE);
        if self.port_map[idx] & map != 1 {
            None
        } else {
            self.port_map[idx] &= !map;
            Some(port)
        }
    }

    /// エフェメラルポート範囲 45152 - 65535 の最初の空き番号を返す。
    /// この関数はポートをマークしないことに注意されたい。
    pub fn get_ephemeral_port(&mut self) -> Option<u16> {
        // Lab 5 2.B
        for port in 45152..=65535  {
            let idx = port / PORT_MAP_SIZE;
            let map: u64 = 1_u64 << (port % PORT_MAP_SIZE);
            if self.port_map[idx] & map == 0 {
                return Some(port as u16);
            }
        }
        None
    }

    /// `SocketHandle` を持つソケットを検索する.
    pub fn get_socket(&mut self, handle: SocketHandle) -> SocketRef<'_, TcpSocket> {
        self.socket_set.get::<TcpSocket>(handle)
    }

    /// 新規TCPソケットを作成して内部ソケットセットに追加し、
    /// 新規ソケットの`SocketHandle`を返す
    /// set, and returns the `SocketHandle` of the new socket.
    pub fn add_socket(&mut self) -> SocketHandle {
        let rx_buffer = TcpSocketBuffer::new(vec![0; 16384]);
        let tx_buffer = TcpSocketBuffer::new(vec![0; 16384]);
        let tcp_socket = TcpSocket::new(rx_buffer, tx_buffer);
        self.socket_set.add(tcp_socket)
    }

    /// 内部ソケットセットからソケットを解放する.
    pub fn release(&mut self, handle: SocketHandle) {
        self.socket_set.release(handle);
    }

    /// 内部ソケットセットを削除する.
    pub fn prune(&mut self) {
        self.socket_set.prune();
    }
}

/// A thread-safe wrapper for `EthernetDriver`.
pub struct GlobalEthernetDriver(Mutex<Option<EthernetDriver>>);

impl GlobalEthernetDriver {
    pub const fn uninitialized() -> GlobalEthernetDriver {
        GlobalEthernetDriver(Mutex::new(None))
    }

    pub fn initialize(&self) {
        let mut lock = self.0.lock();
        *lock = Some(EthernetDriver::new());
    }

    pub fn poll(&self, timestamp: Instant) {
        // Lab 5 2.B
        // FIXME lator
        if aarch64::affinity() == 0 && get_preemptive_counter() > 0 {
            self.0
            .lock()
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .poll(timestamp)
        }
    }

    pub fn poll_delay(&self, timestamp: Instant) -> Duration {
        self.0
            .lock()
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .poll_delay(timestamp)
    }

    pub fn mark_port(&self, port: u16) -> Option<u16> {
        self.0
            .lock()
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .mark_port(port)
    }

    pub fn get_ephemeral_port(&self) -> Option<u16> {
        self.0
            .lock()
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .get_ephemeral_port()
    }

    pub fn add_socket(&self) -> SocketHandle {
        self.0
            .lock()
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .add_socket()
    }

    /// クリティカルリージョンに入り、ソケットへの可変参照を持つ
    /// 指定されたクロージャを実行する
    pub fn with_socket<F, R>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&mut SocketRef<'_, TcpSocket>) -> R,
    {
        let mut guard = self.0.lock();
        let mut socket = guard
            .as_mut()
            .expect("Uninitialized EthernetDriver")
            .get_socket(handle);

        f(&mut socket)
    }

    /// クリティカルリージョンに入り、内部ethernetドライバへの可変参照を持つ
    /// 指定されたクロージャを実行する
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut EthernetDriver) -> R,
    {
        let mut guard = self.0.lock();
        let mut ethernet = guard.as_mut().expect("Uninitialized EthernetDriver");

        f(&mut ethernet)
    }
}
