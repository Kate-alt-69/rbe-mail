use std::time::{SystemTime, UNIX_EPOCH};

const K: [u32; 64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
];

fn rotr(x: u32, n: u32) -> u32 { (x >> n) | (x << (32 - n)) }

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = [0x6a09e667u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (msg.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 { msg.push(0); }
    msg.extend_from_slice(&bits.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
        }
        for i in 16..64 {
            let s0 = rotr(w[i-15],7) ^ rotr(w[i-15],18) ^ (w[i-15] >> 3);
            let s1 = rotr(w[i-2],17) ^ rotr(w[i-2],19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let (mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut hh)=(h[0],h[1],h[2],h[3],h[4],h[5],h[6],h[7]);
        for i in 0..64 {
            let s1=rotr(e,6)^rotr(e,11)^rotr(e,25);
            let ch=(e&f)^((!e)&g);
            let t1=hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0=rotr(a,2)^rotr(a,13)^rotr(a,22);
            let maj=(a&b)^(a&c)^(b&c);
            let t2=s0.wrapping_add(maj);
            hh=g; g=f; f=e; e=d.wrapping_add(t1); d=c; c=b; b=a; a=t1.wrapping_add(t2);
        }
        h[0]=h[0].wrapping_add(a); h[1]=h[1].wrapping_add(b); h[2]=h[2].wrapping_add(c); h[3]=h[3].wrapping_add(d);
        h[4]=h[4].wrapping_add(e); h[5]=h[5].wrapping_add(f); h[6]=h[6].wrapping_add(g); h[7]=h[7].wrapping_add(hh);
    }
    let mut out=[0u8;32];
    for (i,v) in h.iter().enumerate() { out[i*4..i*4+4].copy_from_slice(&v.to_be_bytes()); }
    out
}

pub fn hex(data: &[u8]) -> String {
    const H:&[u8;16]=b"0123456789abcdef";
    let mut out=String::with_capacity(data.len()*2);
    for &b in data { out.push(H[(b>>4) as usize] as char); out.push(H[(b&15) as usize] as char); }
    out
}

pub fn hmac(key:&[u8], data:&[u8])->[u8;32]{
    let mut kb=[0u8;64];
    if key.len()>64 { kb[..32].copy_from_slice(&sha256(key)); } else { kb[..key.len()].copy_from_slice(key); }
    let mut i=[0x36u8;64]; let mut o=[0x5cu8;64];
    for x in 0..64 { i[x]^=kb[x]; o[x]^=kb[x]; }
    let mut a=i.to_vec(); a.extend_from_slice(data); let ih=sha256(&a);
    let mut b=o.to_vec(); b.extend_from_slice(&ih); sha256(&b)
}

fn civil(z:i64)->(i32,u32,u32){
    let z=z+719468; let era=if z>=0{z}else{z-146096}/146097;
    let doe=(z-era*146097) as u32; let yoe=(doe-doe/1460+doe/36524-doe/146096)/365;
    let y=yoe as i32+era as i32*400; let doy=doe-(365*yoe+yoe/4-yoe/100);
    let mp=(5*doy+2)/153; let d=doy-(153*mp+2)/5+1; let m=mp as i32+if mp<10{3}else{-9};
    (y+if m<=2{1}else{0},m as u32,d)
}

fn timestamp()->Result<(String,String),String>{
    let s=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_|"MAIL9002: invalid system clock")?.as_secs() as i64;
    let (y,m,d)=civil(s.div_euclid(86400)); let r=s.rem_euclid(86400);
    let hh=r/3600; let mm=(r%3600)/60; let ss=r%60;
    let date=format!("{y:04}{m:02}{d:02}");
    Ok((format!("{date}T{hh:02}{mm:02}{ss:02}Z"),date))
}

pub fn sign(
    access:&str, secret:&str, token:Option<&str>, region:&str, host:&str,
    method:&str, uri:&str, content_type:&str, body:&[u8]
)->Result<Vec<(&'static str,String)>,String>{
    let (amz,date)=timestamp()?;
    let hash=hex(&sha256(body));
    let mut canonical=format!("content-type:{content_type}\nhost:{host}\nx-amz-date:{amz}\n");
    let mut signed=String::from("content-type;host;x-amz-date");
    if let Some(t)=token { canonical.push_str(&format!("x-amz-security-token:{t}\n")); signed.push_str(";x-amz-security-token"); }
    let req=format!("{method}\n{uri}\n\n{canonical}\n{signed}\n{hash}");
    let scope=format!("{date}/{region}/ses/aws4_request");
    let sts=format!("AWS4-HMAC-SHA256\n{amz}\n{scope}\n{}",hex(&sha256(req.as_bytes())));
    let kd=hmac(format!("AWS4{secret}").as_bytes(),date.as_bytes());
    let kr=hmac(&kd,region.as_bytes()); let ks=hmac(&kr,b"ses"); let ksign=hmac(&ks,b"aws4_request");
    let signature=hex(&hmac(&ksign,sts.as_bytes()));
    let auth=format!("AWS4-HMAC-SHA256 Credential={access}/{scope}, SignedHeaders={signed}, Signature={signature}");
    let mut headers=vec![("Authorization",auth),("X-Amz-Date",amz)];
    if let Some(t)=token { headers.push(("X-Amz-Security-Token",t.to_string())); }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_abc() {
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
