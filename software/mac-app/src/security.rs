//! Wire-compatible client for Espressif Protocomm Security 2 (SRP-3072/SHA512 + AES-GCM).
//! Field numbers follow ESP-IDF sec2.proto/session.proto; no Security 0/1 fallback.
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::{Result, bail, ensure};
use num_bigint::BigUint;
use prost::Message;
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

const N_HEX: &str = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3BE39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA051015728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64ECFB850458DBEF0A8AEA71575D060C7DB3970F85A6E1E4C7ABF5AE8CDB0933D71E8C94E04A25619DCEE3D2261AD2EE6BF12FFA06D98A0864D87602733EC86A64521F2B18177B200CBBE117577A615D6C770988C0BAD946E208E24FA074E5AB3143DB5BFCE0FD108E4B82D120A93AD2CAFFFFFFFFFFFFFFFF";
fn hash(parts: &[&[u8]]) -> Vec<u8> {
    let mut h = Sha512::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().to_vec()
}
fn padded(v: &BigUint) -> Vec<u8> {
    let b = v.to_bytes_be();
    let mut p = vec![0; 384 - b.len()];
    p.extend(b);
    p
}

#[derive(Clone, PartialEq, Message)]
struct Cmd0 {
    #[prost(bytes = "vec", tag = "1")]
    username: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    public: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Resp0 {
    #[prost(int32, tag = "1")]
    status: i32,
    #[prost(bytes = "vec", tag = "2")]
    public: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    salt: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Cmd1 {
    #[prost(bytes = "vec", tag = "1")]
    proof: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Resp1 {
    #[prost(int32, tag = "1")]
    status: i32,
    #[prost(bytes = "vec", tag = "2")]
    proof: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    nonce: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Payload {
    #[prost(int32, tag = "1")]
    msg: i32,
    #[prost(message, optional, tag = "20")]
    c0: Option<Cmd0>,
    #[prost(message, optional, tag = "21")]
    r0: Option<Resp0>,
    #[prost(message, optional, tag = "22")]
    c1: Option<Cmd1>,
    #[prost(message, optional, tag = "23")]
    r1: Option<Resp1>,
}
#[derive(Clone, PartialEq, Message)]
struct Session {
    #[prost(int32, tag = "2")]
    version: i32,
    #[prost(message, optional, tag = "12")]
    payload: Option<Payload>,
}
fn encode(payload: Payload) -> Vec<u8> {
    Session {
        version: 2,
        payload: Some(payload),
    }
    .encode_to_vec()
}
fn decode(raw: &[u8], kind: i32) -> Result<Payload> {
    ensure!(raw.len() <= 2048, "认证回复过长。");
    let s = Session::decode(raw)?;
    ensure!(s.version == 2, "拒绝降低蓝牙安全版本。");
    let p = s.payload.ok_or_else(|| anyhow::anyhow!("缺少认证回复。"))?;
    ensure!(p.msg == kind, "认证步骤不匹配。");
    Ok(p)
}

pub struct Handshake {
    n: BigUint,
    a: BigUint,
    public: Vec<u8>,
    key: Vec<u8>,
    expected: Vec<u8>,
}
impl Drop for Handshake {
    fn drop(&mut self) {
        self.key.zeroize();
        self.expected.zeroize();
        self.a = BigUint::default();
    }
}
impl Handshake {
    pub fn new() -> Self {
        let n = BigUint::parse_bytes(N_HEX.as_bytes(), 16).unwrap();
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        seed[0] |= 0x80;
        let a = BigUint::from_bytes_be(&seed);
        seed.zeroize();
        let public = padded(&BigUint::from(5u8).modpow(&a, &n));
        Self {
            n,
            a,
            public,
            key: vec![],
            expected: vec![],
        }
    }
    pub fn first(&self, username: &str) -> Vec<u8> {
        encode(Payload {
            msg: 0,
            c0: Some(Cmd0 {
                username: username.as_bytes().into(),
                public: self.public.clone(),
            }),
            ..Default::default()
        })
    }
    pub fn challenge(&mut self, raw: &[u8], username: &str, password: &str) -> Result<Vec<u8>> {
        let r = decode(raw, 1)?
            .r0
            .ok_or_else(|| anyhow::anyhow!("无设备挑战。"))?;
        ensure!(
            r.status == 0 && !r.salt.is_empty() && r.salt.len() <= 64 && r.public.len() == 384,
            "设备认证挑战无效。"
        );
        let b = BigUint::from_bytes_be(&r.public);
        ensure!(&b % &self.n != BigUint::default(), "无效 SRP 公钥。");
        let g = BigUint::from(5u8);
        let k = BigUint::from_bytes_be(&hash(&[&padded(&self.n), &padded(&g)]));
        let u = BigUint::from_bytes_be(&hash(&[&self.public, &r.public]));
        ensure!(u != BigUint::default(), "无效 SRP 挑战。");
        let mut identity = Vec::from(username.as_bytes());
        identity.push(b':');
        identity.extend(password.as_bytes());
        let mut inner = hash(&[&identity]);
        identity.zeroize();
        let mut xbytes = hash(&[&r.salt, &inner]);
        inner.zeroize();
        let x = BigUint::from_bytes_be(&xbytes);
        xbytes.zeroize();
        let kv = (&k * g.modpow(&x, &self.n)) % &self.n;
        let base = (&b + &self.n - kv) % &self.n;
        let shared = base.modpow(&(&self.a + u * x), &self.n);
        let mut shared_bytes = shared.to_bytes_be();
        self.key = hash(&[&shared_bytes]);
        shared_bytes.zeroize();
        let hn = hash(&[&padded(&self.n)]);
        let hg = hash(&[&padded(&g)]);
        let xor: Vec<u8> = hn.iter().zip(hg).map(|(a, b)| a ^ b).collect();
        let proof = hash(&[
            &xor,
            &hash(&[username.as_bytes()]),
            &r.salt,
            &self.public,
            &r.public,
            &self.key,
        ]);
        self.expected = hash(&[&self.public, &proof, &self.key]);
        Ok(encode(Payload {
            msg: 2,
            c1: Some(Cmd1 { proof }),
            ..Default::default()
        }))
    }
    pub fn finish(&self, raw: &[u8]) -> Result<Cipher> {
        let r = decode(raw, 3)?
            .r1
            .ok_or_else(|| anyhow::anyhow!("缺少设备证明。"))?;
        ensure!(
            r.status == 0
                && r.proof.len() == 64
                && self.expected.len() == 64
                && bool::from(r.proof.ct_eq(&self.expected)),
            "设备认证失败，配对资料不匹配。"
        );
        ensure!(
            r.nonce.len() == 12 && self.key.len() == 64,
            "无效会话参数。"
        );
        let mut nonce = [0; 12];
        nonce.copy_from_slice(&r.nonce);
        Ok(Cipher {
            aes: Aes256Gcm::new_from_slice(&self.key[..32]).unwrap(),
            nonce,
        })
    }
}
pub struct Cipher {
    aes: Aes256Gcm,
    nonce: [u8; 12],
}
impl Cipher {
    fn increment(&mut self) -> Result<()> {
        let c = u32::from_be_bytes(self.nonce[8..].try_into().unwrap());
        let Some(n) = c.checked_add(1) else {
            bail!("会话计数器已用尽，请重连。");
        };
        self.nonce[8..].copy_from_slice(&n.to_be_bytes());
        Ok(())
    }
    pub fn encrypt(&mut self, raw: &[u8]) -> Result<Vec<u8>> {
        let out = self
            .aes
            .encrypt(Nonce::from_slice(&self.nonce), raw)
            .map_err(|_| anyhow::anyhow!("蓝牙加密失败。"))?;
        self.increment()?;
        Ok(out)
    }
    pub fn decrypt(&mut self, raw: &[u8]) -> Result<Vec<u8>> {
        let out = self
            .aes
            .decrypt(Nonce::from_slice(&self.nonce), raw)
            .map_err(|_| anyhow::anyhow!("蓝牙回复认证失败。"))?;
        self.increment()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_python_interop_with_leading_zero_digest_and_salt() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../tests/security2-fixture.json")).unwrap();
        let hex = |name: &str| {
            let s = v[name].as_str().unwrap();
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect::<Vec<u8>>()
        };
        let mut h = Handshake::new();
        h.a = BigUint::from_bytes_be(&hex("a_hex"));
        h.public = padded(&BigUint::from(5u8).modpow(&h.a, &h.n));
        let r0 = encode(Payload {
            msg: 1,
            r0: Some(Resp0 {
                status: 0,
                public: hex("B_hex"),
                salt: hex("salt_hex"),
            }),
            ..Default::default()
        });
        let cmd = h
            .challenge(
                &r0,
                v["username"].as_str().unwrap(),
                v["password"].as_str().unwrap(),
            )
            .unwrap();
        assert_eq!(decode(&cmd, 2).unwrap().c1.unwrap().proof, hex("M_hex"));
        let r1 = encode(Payload {
            msg: 3,
            r1: Some(Resp1 {
                status: 0,
                proof: hex("proof_hex"),
                nonce: hex("nonce_hex"),
            }),
            ..Default::default()
        });
        assert!(h.finish(&r1).is_ok());
        let bad = encode(Payload {
            msg: 3,
            r1: Some(Resp1 {
                status: 0,
                proof: vec![0; 64],
                nonce: hex("nonce_hex"),
            }),
            ..Default::default()
        });
        assert!(h.finish(&bad).is_err());
    }
    #[test]
    fn group_and_counter() {
        let h = Handshake::new();
        assert_eq!(h.n.bits(), 3072);
        assert_eq!(h.public.len(), 384);
        let mut c = Cipher {
            aes: Aes256Gcm::new_from_slice(&[1; 32]).unwrap(),
            nonce: [0; 12],
        };
        let mut d = Cipher {
            aes: Aes256Gcm::new_from_slice(&[1; 32]).unwrap(),
            nonce: [0; 12],
        };
        let encrypted = c.encrypt(b"?").unwrap();
        assert_eq!(d.decrypt(&encrypted).unwrap(), b"?");
        assert!(d.decrypt(&encrypted).is_err());
    }
    #[test]
    fn reject_downgrade() {
        assert!(
            decode(
                &Session {
                    version: 1,
                    payload: Some(Payload::default())
                }
                .encode_to_vec(),
                0
            )
            .is_err()
        );
    }
}
