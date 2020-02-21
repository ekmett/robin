#![allow(dead_code)]
use rand::seq::SliceRandom;
use rand::thread_rng;
use raptorq::*;
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::default::Default;
use std::io::{self,Write};
use std::collections::HashMap;

pub const MTU: u16 = 1200;

/// meta data about a file being transferred
#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Debug)]
pub struct Meta {
  filename: String,
  oti: ObjectTransmissionInformation
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Sender {
  systematic: bool,
  initial_repairs: u32,
  residual_batch_size: u32,
  starting_offset: u32,
  shuffle: bool
}

impl Default for Sender {
  fn default() -> Self { Sender::new() }
}

impl Sender {
  pub fn new() -> Self {
    Sender {
      systematic: true,
      initial_repairs: 0,
      residual_batch_size: 1,
      starting_offset: 0,
      shuffle: true
    }
  }
  pub fn send<F>(
    &self,
    filename: String,
    data: &[u8],
    handler: F
  ) where F : Fn(&[u8]) -> () {
    let mut rng = thread_rng();
    let encoder = Encoder::with_defaults(data, MTU);
    let meta = Meta { filename: filename.clone(), oti: encoder.get_config() };

    if self.systematic {
      println!("{} systematic",&filename); io::stdout().flush().unwrap();
      // systematic initial transmission, including initial check data per batch
      let ps = encoder.get_encoded_packets(self.initial_repairs);
      let mut packet_ids: Vec<usize> = (0..ps.len()).collect();
      if self.shuffle {
        packet_ids.shuffle(&mut rng);
      }
      let mut counter = 0;
      for id in packet_ids {
        let data = serde_cbor::to_vec(&(&meta,&ps[id])).expect("cbor");
        let digest = Sha256::digest(&data);
        let packet = serde_cbor::to_vec(&(data,digest.as_slice())).expect("cbor");
        handler(&packet);
        counter+=1;
        print!("\r{}/{} packets",counter,ps.len());
      }
    }

    println!("");

    // an endless fountain of residual packets
    let chunks = encoder.get_block_encoders();
    let mut chunk_ids: Vec<usize> = (0..chunks.len()).collect();

    let mut shard = self.starting_offset;
    loop {
      if self.shuffle {
        chunk_ids.shuffle(&mut rng);
      }
      for chunk_id in chunk_ids.iter() {
        for packet in chunks[*chunk_id].repair_packets(shard,self.residual_batch_size).iter() {
          let data = serde_cbor::to_vec(&(&meta,packet)).expect("cbor");
          let digest = Sha256::digest(&data);
          let packet = serde_cbor::to_vec(&(data,digest.as_slice())).expect("cbor");
          handler(&packet);
        }
      }
      shard += self.residual_batch_size;
      print!("\r{} ({} packet) repair passes",shard,chunk_ids.len()); io::stdout().flush().unwrap();
    }
  }
}

/// TODO: a fancy progress meter?
pub struct Receiver {
  decoders: HashMap<Meta,Option<Decoder>>
}

impl Receiver {
  pub fn new() -> Self {
    Receiver {
      decoders: HashMap::new()
    }
  }

  // returns true if we handled this packet and can kill this decoder
  pub fn recv<F>(
    &mut self,
    data: &[u8],
    handler: F
  ) -> Option<bool> where F : Fn(Meta,Vec<u8>) -> () {
    let (pm,digest): (Vec<u8>,Vec<u8>) = serde_cbor::from_slice(data).expect("valid data"); // .ok()?;
    let actual_digest = Sha256::digest(&pm);
    if digest != actual_digest.as_slice() {
      println!("Bad Digest: {:?} /= {:?}", digest, actual_digest);
      return None;
    }
    let (meta,packet): (Meta,EncodingPacket) = serde_cbor::from_slice(&pm).expect("valid data"); // .ok()?;
    if !self.decoders.contains_key(&meta) {
      println!("Starting {} ({} bytes):",meta.filename.clone(),meta.oti.transfer_length);
      self.decoders.insert(meta.clone(),Some(Decoder::new(meta.oti.clone())));
    }
    let slot = self.decoders.entry(meta.clone()).or_insert_with(|| Some(Decoder::new(meta.oti.clone())));
    match slot {
      None => return Some(false),
      Some(decoder) =>
        if let Some(content) = decoder.decode(packet) {
          println!("{} complete",meta.filename);
          handler(meta,content);
          *slot = None;
          return Some(true);
        } else {
          let (n,d) = decoder.get_progress();
          println!("{}: {}/{} blocks received",meta.filename,n,d);
          return Some(false);
        }
    }
  }
}
