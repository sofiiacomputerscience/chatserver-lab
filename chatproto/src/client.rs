use crate::{
  core::WORKPROOF_STRENGTH,
  messages::{ClientId, Sequence},
  workproof::gen_workproof,
};

#[derive(Debug, Default)]
pub struct Client {
  id: ClientId,
  curid: u128,
}

impl Client {
  pub fn new(id: ClientId) -> Self {
    Client { id, curid: 0 }
  }
  pub fn sequence<A>(&mut self, content: A) -> Sequence<A> {
    self.curid += 1;
    let workproof = gen_workproof((&self.id).into(), WORKPROOF_STRENGTH, u128::MAX).unwrap();
    Sequence {
      seqid: self.curid,
      src: self.id,
      workproof,
      content,
    }
  }
}
