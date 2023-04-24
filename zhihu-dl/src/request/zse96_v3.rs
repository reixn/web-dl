use md5::{Digest, Md5};
use reqwest::{IntoUrl, Method, RequestBuilder};
use std::{array::from_fn, mem::MaybeUninit};

fn g(e: u32) -> u32 {
    const H_ZB: [u8; 256] = [
        0x14, 0xdf, 0xf5, 0x07, 0xf8, 0x02, 0xc2, 0xd1, 0x57, 0x06, 0xe3, 0xfd, 0xf0, 0x80, 0xde,
        0x5b, 0xed, 0x09, 0x7d, 0x9d, 0xe6, 0x5d, 0xfc, 0xcd, 0x5a, 0x4f, 0x90, 0xc7, 0x9f, 0xc5,
        0xba, 0xa7, 0x27, 0x25, 0x9c, 0xc6, 0x26, 0x2a, 0x2b, 0xa8, 0xd9, 0x99, 0x0f, 0x67, 0x50,
        0xbd, 0x47, 0xbf, 0x61, 0x54, 0xf7, 0x5f, 0x24, 0x45, 0x0e, 0x23, 0x0c, 0xab, 0x1c, 0x72,
        0xb2, 0x94, 0x56, 0xb6, 0x20, 0x53, 0x9e, 0x6d, 0x16, 0xff, 0x5e, 0xee, 0x97, 0x55, 0x4d,
        0x7c, 0xfe, 0x12, 0x04, 0x1a, 0x7b, 0xb0, 0xe8, 0xc1, 0x83, 0xac, 0x8f, 0x8e, 0x96, 0x1e,
        0x0a, 0x92, 0xa2, 0x3e, 0xe0, 0xda, 0xc4, 0xe5, 0x01, 0xc0, 0xd5, 0x1b, 0x6e, 0x38, 0xe7,
        0xb4, 0x8a, 0x6b, 0xf2, 0xbb, 0x36, 0x78, 0x13, 0x2c, 0x75, 0xe4, 0xd7, 0xcb, 0x35, 0xef,
        0xfb, 0x7f, 0x51, 0x0b, 0x85, 0x60, 0xcc, 0x84, 0x29, 0x73, 0x49, 0x37, 0xf9, 0x93, 0x66,
        0x30, 0x7a, 0x91, 0x6a, 0x76, 0x4a, 0xbe, 0x1d, 0x10, 0xae, 0x05, 0xb1, 0x81, 0x3f, 0x71,
        0x63, 0x1f, 0xa1, 0x4c, 0xf6, 0x22, 0xd3, 0x0d, 0x3c, 0x44, 0xcf, 0xa0, 0x41, 0x6f, 0x52,
        0xa5, 0x43, 0xa9, 0xe1, 0x39, 0x70, 0xf4, 0x9b, 0x33, 0xec, 0xc8, 0xe9, 0x3a, 0x3d, 0x2f,
        0x64, 0x89, 0xb9, 0x40, 0x11, 0x46, 0xea, 0xa3, 0xdb, 0x6c, 0xaa, 0xa6, 0x3b, 0x95, 0x34,
        0x69, 0x18, 0xd4, 0x4e, 0xad, 0x2d, 0x00, 0x74, 0xe2, 0x77, 0x88, 0xce, 0x87, 0xaf, 0xc3,
        0x19, 0x5c, 0x79, 0xd0, 0x7e, 0x8b, 0x03, 0x4b, 0x8d, 0x15, 0x82, 0x62, 0xf1, 0x28, 0x9a,
        0x42, 0xb8, 0x31, 0xb5, 0x2e, 0xf3, 0x58, 0x65, 0xb7, 0x08, 0x17, 0x48, 0xbc, 0x68, 0xb3,
        0xd2, 0x86, 0xfa, 0xc9, 0xa4, 0x59, 0xd8, 0xca, 0xdc, 0x32, 0xdd, 0x98, 0x8c, 0x21, 0xeb,
        0xd6,
    ];
    fn q(e: u32, t: u32) -> u32 {
        ((e as i32) << t) as u32 | e >> (32 - t)
    }
    let r = u32::from_be_bytes(e.to_be_bytes().map(|i| H_ZB[i as usize]));
    r ^ q(r, 2) ^ q(r, 10) ^ q(r, 18) ^ q(r, 24)
}
fn g_r(e: &[u8; 16], dest: &mut [MaybeUninit<u8>; 16]) {
    const H_ZK: [u32; 32] = [
        0x45c62932, 0x3d15f2fe, 0x5442e14f, 0xeb8921c0, 0xd256542e, 0xae28cbde, 0xf7782b08,
        0xee48a883, 0x733e8d1a, 0xc61cdffb, 0xe7c6016a, 0x1b713876, 0xdf5eeb0a, 0x8f44a6ca,
        0x9beb07a3, 0x7e564e94, 0x870bcbcb, 0x794d026c, 0xa54f723a, 0xffaabf19, 0xfb5d9cc3,
        0x832a8363, 0xb5e884fa, 0x5e2b60cf, 0x4ec93b52, 0x1b3a7714, 0xad0d330f, 0xf2551fdf,
        0x13ab7196, 0xd0f96ade, 0x15ab9f7d, 0x8be5d87b,
    ];
    let mut n: [MaybeUninit<u32>; 36] = MaybeUninit::uninit_array();
    for i in 0..4 {
        n[i].write(u32::from_be_bytes([
            e[i * 4],
            e[i * 4 + 1],
            e[i * 4 + 2],
            e[i * 4 + 3],
        ]));
    }
    for i in 0..32 {
        n[i + 4].write(unsafe {
            n[i].assume_init()
                ^ g(n[i + 1].assume_init()
                    ^ n[i + 2].assume_init()
                    ^ n[i + 3].assume_init()
                    ^ H_ZK[i])
        });
    }
    let n = unsafe { MaybeUninit::array_assume_init(n) };
    MaybeUninit::write_slice(&mut dest[0..4], &n[35].to_be_bytes());
    MaybeUninit::write_slice(&mut dest[4..8], &n[34].to_be_bytes());
    MaybeUninit::write_slice(&mut dest[8..12], &n[33].to_be_bytes());
    MaybeUninit::write_slice(&mut dest[12..16], &n[32].to_be_bytes());
}
fn g_x(e: &[u8; 32], t: &[u8; 16], dest: &mut [MaybeUninit<u8>; 32]) {
    let mut v: [u8; 16] = from_fn(|i| e[i] ^ t[i]);
    g_r(&v, (&mut dest[0..16]).try_into().unwrap());
    for i in 0..16 {
        v[i] = e[i + 16] ^ unsafe { dest[i].assume_init() };
    }
    g_r(&v, (&mut dest[16..32]).try_into().unwrap());
}

fn encode_zse96(d: &[u8; 16]) -> String {
    let l53: [u8; 48] = {
        let mut l50: [MaybeUninit<u8>; 48] = MaybeUninit::uninit_array();
        l50[0].write(63);
        l50[1].write(0);
        for i in 0..16 {
            MaybeUninit::write_slice(
                &mut l50[2 + i * 2..2 + i * 2 + 2],
                &base16::encode_byte(d[i], base16::EncodeLower),
            );
        }
        for i in &mut l50[34..48] {
            i.write(48 - 34);
        }
        let l50 = unsafe { MaybeUninit::array_assume_init(l50) };

        let mut ret: [MaybeUninit<u8>; 48] = MaybeUninit::uninit_array();

        g_r(
            &{
                const TABLE: [u8; 16] = [
                    0x30, 0x35, 0x39, 0x30, 0x35, 0x33, 0x66, 0x37, 0x64, 0x31, 0x35, 0x65, 0x30,
                    0x31, 0x64, 0x37,
                ];
                let l34 = &l50[0..16];
                from_fn::<u8, 16, _>(|i| l34[i] ^ TABLE[i] ^ 42)
            },
            (&mut ret[0..16]).try_into().unwrap(),
        );
        let (l36, l39) = unsafe {
            (
                (ret.as_ptr() as *const [u8; 16]).as_ref().unwrap(),
                (ret[16..].as_mut_ptr() as *mut [MaybeUninit<u8>; 32])
                    .as_mut()
                    .unwrap(),
            )
        };
        g_x((&l50[16..48]).try_into().unwrap(), l36, l39);

        unsafe { MaybeUninit::array_assume_init(ret) }
    };
    const TABLE_55: [char; 65] = [
        '6', 'f', 'p', 'L', 'R', 'q', 'J', 'O', '8', 'M', '/', 'c', '3', 'j', 'n', 'Y', 'x', 'F',
        'k', 'U', 'V', 'C', '4', 'Z', 'I', 'G', '1', '2', 'S', 'i', 'H', '=', '5', 'v', '0', 'm',
        'X', 'D', 'a', 'z', 'W', 'B', 'T', 's', 'u', 'w', '7', 'Q', 'e', 't', 'b', 'K', 'd', 'o',
        'P', 'y', 'A', 'l', '+', 'h', 'N', '9', 'r', 'g', 'E',
    ];
    let mut l56 = 0;
    let mut l57 = String::from("2.0_");
    l57.reserve(64);
    for l13 in (0..48).rev().step_by(3) {
        fn l58(l53: &[u8], l56: i32, i: usize) -> usize {
            let l58 = (8 * (l56 & 0x3)) % 32;
            ((l53[i] as u32) ^ (58 >> l58) & 255) as usize
        }
        let mut l59 = l58(&l53, l56, l13);
        l56 += 1;
        l59 |= l58(&l53, l56, l13 - 1) << 8;
        l56 += 1;
        l59 |= l58(&l53, l56, l13 - 2) << 16;
        l56 += 1;
        l57.push(TABLE_55[l59 & 63]);
        l57.push(TABLE_55[(l59 >> 6) & 63]);
        l57.push(TABLE_55[(l59 >> 12) & 63]);
        l57.push(TABLE_55[(l59 >> 18) & 63]);
    }
    l57
}

pub struct Zse96V3;

impl super::Signer for Zse96V3 {
    fn sign_request<U: IntoUrl>(client: &super::Client, method: Method, path: U) -> RequestBuilder {
        let mut dig = Md5::new_with_prefix("101_3_3.0+");
        let url = path.into_url().unwrap();
        dig.update(url.path());
        if let Some(q) = url.query() {
            dig.update("?");
            dig.update(q);
        }
        dig.update("+");
        dig.update(
            client
                .cookie_store
                .lock()
                .unwrap()
                .get("zhihu.com", "/", "d_c0")
                .unwrap()
                .value(),
        );
        let enc = encode_zse96(&dig.finalize().into());
        log::debug!("request {} signature: {}", url, enc);
        client
            .http_client
            .request(method, url)
            .header("x-zse-93", "101_3_3.0")
            .header("x-zse-96", enc)
    }
}

#[cfg(test)]
mod tests {
    use super::encode_zse96;
    use hex_literal::hex;

    #[test]
    fn encode() {
        assert_eq!(
            encode_zse96(&hex!("202cb962ac59075b964b07152d234b70")),
            "2.0_Bj7+myTibavoPUJRzaV2S1uvi9kx+mqtQcQH9TxBRpiBkgNkW8z86R52P5jL3f=r"
        );
    }
}
