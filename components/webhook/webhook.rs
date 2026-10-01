#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum DeliveryEvent{Accepted,Delivered,Deferred,Bounced,Complained,Opened,Clicked,Unknown}
pub fn normalize(_provider:&str,event:&str)->DeliveryEvent{
    match event.to_ascii_lowercase().as_str(){
        "delivered"|"delivery"=>DeliveryEvent::Delivered,
        "bounce"|"bounced"|"permanent_fail"=>DeliveryEvent::Bounced,
        "complaint"|"complained"|"spamreport"=>DeliveryEvent::Complained,
        "open"|"opened"=>DeliveryEvent::Opened,
        "click"|"clicked"=>DeliveryEvent::Clicked,
        "deferred"|"delay"|"temporary_fail"=>DeliveryEvent::Deferred,
        "accepted"|"sent"|"processed"|"queued"=>DeliveryEvent::Accepted,
        _=>DeliveryEvent::Unknown,
    }
}
