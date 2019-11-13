extern crate serde;
extern crate serde_json;

use analyze::sequence::SequenceReport;
use std::io::{Read, Write};

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlMessage {
    RequestFlow,
    ExpectFlow(u16),
    TerminateFlow(u16),
    Report(SequenceReport),
}

pub trait ControlStream {
    fn send_msg(&mut self, ControlMessage) -> Result<(), String>;
    fn recv_msg(&mut self) -> Result<ControlMessage, String>;
}

impl<T> ControlStream for T
where
    T: Read + Write,
{
    fn send_msg(&mut self, msg: ControlMessage) -> Result<(), String> {
        let mut data = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
        data.push(0);
        self.write(&data)
            .and(self.flush())
            .map_err(|e| e.to_string())
            .map(|_| ())
    }

    fn recv_msg(&mut self) -> Result<ControlMessage, String> {
        use std::io::{BufRead, BufReader};
        let mut buf_stream = BufReader::new(self);
        let mut message_data: Vec<u8> = Vec::new();

        let bytes = buf_stream
            .read_until(0, &mut message_data)
            .map_err(|e| e.to_string())?;

        if bytes == 0 || message_data[bytes - 1] != 0 {
            // short read due to EOF
            return Err(
                "Control connection closed by remote side".to_string()
            );
        } else {
            message_data.pop();
            let message: ControlMessage =
                serde_json::from_slice(&message_data)
                    .map_err(|e| e.to_string())?;
            Ok(message)
        }
    }
}
