use bytes::Bytes;
use resp_rs::resp2::parse_frame;

fn main() {
    let input = b"*3\r\n$3\r\nPUT\r\n$5\r\nmykey\r\n$5\r\nmyval\r\n";
    let bytes = Bytes::from_static(input);
    match parse_frame(bytes) {
        Ok((frame, remaining)) => {
            println!("Success! Frame: {:?}", frame);
            println!("Remaining bytes: {:?}", remaining);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
