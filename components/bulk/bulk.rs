pub fn chunks<T>(items:&[T],max_per_batch:usize)->Result<Vec<&[T]>,String>{
    if max_per_batch==0{return Err("MAIL1002: bulk batch size cannot be zero".into());}
    Ok(items.chunks(max_per_batch).collect())
}
pub fn recommended_batch_size(provider:&str)->usize{
    match provider.to_ascii_lowercase().as_str(){
        "resend"=>100,
        "sendgrid"=>1000,
        _=>100,
    }
}
