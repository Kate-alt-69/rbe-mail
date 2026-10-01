use rbe_sdk::{HostBridge,RbeSdk};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct MxRecord{pub preference:u16,pub exchange:String}

fn objects(input:&str)->Vec<&str>{
    let mut out=Vec::new();let bytes=input.as_bytes();let mut i=0;
    while i<bytes.len(){
        if bytes[i]==b'{'{
            let start=i;let mut depth=1;let mut q=false;let mut esc=false;i+=1;
            while i<bytes.len()&&depth>0{
                let b=bytes[i];
                if q{if esc{esc=false}else if b==b'\\'{esc=true}else if b==b'"'{q=false}}
                else if b==b'"'{q=true}else if b==b'{'{depth+=1}else if b==b'}'{depth-=1;if depth==0{out.push(&input[start..=i]);}}
                i+=1;
            }
        }else{i+=1}
    }out
}
fn u16field(input:&str,name:&str)->Option<u16>{
    let n=format!("\"{name}\"");let t=input.get(input.find(&n)?+n.len()..)?;
    let t=t.get(t.find(':')?+1..)?.trim_start();let e=t.find(|c:char|!c.is_ascii_digit()).unwrap_or(t.len());
    t[..e].parse().ok()
}
fn strfield(input:&str,name:&str)->Option<String>{
    let n=format!("\"{name}\"");let t=input.get(input.find(&n)?+n.len()..)?;
    let t=t.get(t.find(':')?+1..)?.trim_start().strip_prefix('"')?;Some(t[..t.find('"')?].to_string())
}
pub fn mx(host:&dyn HostBridge,domain:&str)->Result<Vec<MxRecord>,String>{
    if domain.trim().is_empty()||domain.contains('/'){return Err("MAIL1015: invalid DNS domain".into());}
    let sdk=RbeSdk::new(host);
    if sdk.host().granted("net:dns")==Some(false){return Err("MAIL2005: RBE net:dns capability is not granted".into());}
    let payload=format!("\"{}\"",domain.replace('\\',"\\\\").replace('"',"\\\""));
    let reply=sdk.net().dns().call("mx",payload.as_bytes()).map_err(|e|format!("MAIL4013: MX lookup failed: {e}"))?;
    let text=std::str::from_utf8(&reply.payload).map_err(|_|"MAIL8002: DNS response was not UTF-8")?;
    let mut result=Vec::new();
    for o in objects(text){
        if let(Some(preference),Some(exchange))=(u16field(o,"preference"),strfield(o,"exchange")){
            result.push(MxRecord{preference,exchange});
        }
    }
    result.sort_by_key(|r|r.preference);
    if result.is_empty(){return Err("MAIL4014: no MX records returned".into());}
    Ok(result)
}
