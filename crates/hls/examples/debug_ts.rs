use bytes::Bytes;
use ts::{TsPacketRef, TsParser};

fn main() {
    println!("Debug TS Parsing");
    println!("================");

    // Create the exact same data as simple_ts_test
    let ts_data = create_working_ts_data();
    let ts_bytes = Bytes::from(ts_data);

    // Parse packets manually to debug
    for (i, chunk) in ts_bytes.chunks(188).enumerate() {
        if chunk.len() < 188 {
            continue;
        }
        println!("\nPacket {i}: ");
        match TsPacketRef::parse(Bytes::copy_from_slice(chunk)) {
            Ok(packet) => {
                println!("  PID: 0x{:04X}", packet.pid);
                println!("  PUSI: {}", packet.payload_unit_start_indicator);
                if let Some(payload) = packet.payload() {
                    println!("  Payload length: {}", payload.len());
                }

                if let Some(psi_payload) = packet.psi_payload() {
                    println!("  PSI payload length: {}", psi_payload.len());
                    if !psi_payload.is_empty() {
                        println!("  Table ID: 0x{:02X}", psi_payload[0]);
                        if psi_payload.len() >= 3 {
                            let section_length =
                                ((psi_payload[1] as u16 & 0x0F) << 8) | psi_payload[2] as u16;
                            println!("  Section length: {section_length}");
                        }
                    }
                }
            }
            Err(e) => {
                println!("  Error: {e}");
            }
        }
    }

    // Now try with TsParser
    println!("\n\nTesting with TsParser:");
    let mut parser = TsParser::new();
    let on_pat = |pat: ts::PatRef| {
        println!("  PAT: programs={}", pat.program_count());
        Ok(())
    };
    let on_pmt = |pmt: ts::PmtRef| {
        println!(
            "  PMT for program {}: streams={}",
            pmt.program_number,
            pmt.streams().count()
        );
        Ok(())
    };

    match parser.parse_packets(
        ts_bytes,
        on_pat,
        on_pmt,
        None::<fn(&ts::TsPacketRef) -> ts::Result<()>>,
    ) {
        Ok(()) => {
            println!("✓ TsParser succeeded");
        }
        Err(e) => {
            println!("❌ TsParser failed: {e}");
        }
    }
}

fn create_working_ts_data() -> Vec<u8> {
    // Use the exact test data format from the ts crate tests
    let mut ts_data = Vec::new();

    // PAT packet (188 bytes)
    let mut pat_packet = vec![0u8; 188];
    pat_packet[0] = 0x47; // Sync byte
    pat_packet[1] = 0x40; // PUSI set, PID = 0 (PAT)
    pat_packet[2] = 0x00;
    pat_packet[3] = 0x10; // No scrambling, payload only, continuity = 0

    // Simple PAT payload (based on ts crate test)
    pat_packet[4] = 0x00; // Pointer field
    pat_packet[5] = 0x00; // Table ID (PAT)
    pat_packet[6] = 0x80; // Section syntax indicator + reserved + section length high
    pat_packet[7] = 0x0D; // Section length low (13 bytes)
    pat_packet[8] = 0x00; // Transport stream ID high
    pat_packet[9] = 0x01; // Transport stream ID low
    pat_packet[10] = 0x01; // Version 0 + current/next = 1
    pat_packet[11] = 0x00; // Section number
    pat_packet[12] = 0x00; // Last section number
    // Program entry
    pat_packet[13] = 0x00; // Program number high
    pat_packet[14] = 0x01; // Program number low (1)
    pat_packet[15] = 0xE1; // PMT PID high (0x100)
    pat_packet[16] = 0x00; // PMT PID low
    // CRC32 (4 bytes) - leaving as zeros for now
    pat_packet[17] = 0x00;
    pat_packet[18] = 0x00;
    pat_packet[19] = 0x00;
    pat_packet[20] = 0x00;

    // PMT packet (188 bytes)
    let mut pmt_packet = vec![0u8; 188];
    pmt_packet[0] = 0x47; // Sync byte
    pmt_packet[1] = 0x41; // PUSI set, PID = 0x100 (PMT)
    pmt_packet[2] = 0x00;
    pmt_packet[3] = 0x10; // No scrambling, payload only, continuity = 0

    // Simple PMT payload
    pmt_packet[4] = 0x00; // Pointer field
    pmt_packet[5] = 0x02; // Table ID (PMT)
    pmt_packet[6] = 0x80; // Section syntax indicator + reserved + section length high
    pmt_packet[7] = 0x12; // Section length low (18 bytes)
    pmt_packet[8] = 0x00; // Program number high
    pmt_packet[9] = 0x01; // Program number low
    pmt_packet[10] = 0x01; // Version 0 + current/next = 1
    pmt_packet[11] = 0x00; // Section number
    pmt_packet[12] = 0x00; // Last section number
    pmt_packet[13] = 0xE1; // PCR PID high (0x100)
    pmt_packet[14] = 0x00; // PCR PID low
    pmt_packet[15] = 0x00; // Program info length high
    pmt_packet[16] = 0x00; // Program info length low
    // Elementary stream
    pmt_packet[17] = 0x1B; // Stream type (H.264)
    pmt_packet[18] = 0xE1; // Elementary PID high (0x100)
    pmt_packet[19] = 0x00; // Elementary PID low
    pmt_packet[20] = 0x00; // ES info length high
    pmt_packet[21] = 0x00; // ES info length low
    // CRC32 (4 bytes)
    pmt_packet[22] = 0x00;
    pmt_packet[23] = 0x00;
    pmt_packet[24] = 0x00;
    pmt_packet[25] = 0x00;

    ts_data.extend_from_slice(&pat_packet);
    ts_data.extend_from_slice(&pmt_packet);

    ts_data
}
