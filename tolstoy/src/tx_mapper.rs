// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use rusqlite;
use uuid::Uuid;

use core_traits::{
    Entid,
};

use public_traits::errors::{
    Result,
};

use tolstoy_traits::errors::{
    TolstoyError,
};

use types::{
    LocalGlobalTxMapping,
};

// Exposes a tx<->uuid mapping interface.
pub struct TxMapper {}

impl TxMapper {
    pub fn set_lg_mappings(db_tx: &mut rusqlite::Transaction, mappings: Vec<LocalGlobalTxMapping>) -> Result<()> {
        let mut stmt = db_tx.prepare_cached(
            "INSERT OR REPLACE INTO tolstoy_tu (tx, uuid) VALUES (?, ?)"
        )?;
        for mapping in mappings.iter() {
            let uuid_bytes = mapping.remote.as_bytes().to_vec();
            stmt.execute(&[&mapping.local, &uuid_bytes])?;
        }
        Ok(())
    }

    pub fn set_lg_mapping(db_tx: &mut rusqlite::Transaction, mapping: LocalGlobalTxMapping) -> Result<()> {
        TxMapper::set_lg_mappings(db_tx, vec![mapping])
    }

    // TODO for when we're downloading, right?
    pub fn get_or_set_uuid_for_tx(db_tx: &mut rusqlite::Transaction, tx: Entid) -> Result<Uuid> {
        match TxMapper::get(db_tx, tx)? {
            Some(uuid) => Ok(uuid),
            None => {
                let uuid = Uuid::new_v4();
                let uuid_bytes = uuid.as_bytes().to_vec();
                db_tx.execute("INSERT INTO tolstoy_tu (tx, uuid) VALUES (?, ?)", &[&tx, &uuid_bytes])?;
                return Ok(uuid);
            }
        }
    }

    pub fn get_tx_for_uuid(db_tx: &rusqlite::Transaction, uuid: &Uuid) -> Result<Option<Entid>> {
        let mut stmt = db_tx.prepare_cached(
            "SELECT tx FROM tolstoy_tu WHERE uuid = ?"
        )?;

        let uuid_bytes = uuid.as_bytes().to_vec();
        let results = stmt.query_map(&[&uuid_bytes], |r| r.get(0))?;

        let mut txs = vec![];
        txs.extend(results);
        if txs.len() == 0 {
            return Ok(None);
        } else if txs.len() > 1 {
            bail!(TolstoyError::TxIncorrectlyMapped(txs.len()));
        }
        Ok(Some(txs.remove(0)?))
    }

    pub fn get(db_tx: &rusqlite::Transaction, tx: Entid) -> Result<Option<Uuid>> {
        let mut stmt = db_tx.prepare_cached(
            "SELECT uuid FROM tolstoy_tu WHERE tx = ?"
        )?;

        let results = stmt.query_and_then(&[&tx], |r| -> Result<Uuid>{
            let bytes: Vec<u8> = r.get(0);
            Uuid::from_bytes(bytes.as_slice()).map_err(|e| e.into())
        })?;

        let mut uuids = vec![];
        uuids.extend(results);
        if uuids.len() == 0 {
            return Ok(None);
        } else if uuids.len() > 1 {
            bail!(TolstoyError::TxIncorrectlyMapped(uuids.len()));
        }
        Ok(Some(uuids.remove(0)?))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use schema;

    #[test]
    fn test_getters() {
        let mut conn = schema::tests::setup_conn_bare();
        let mut tx = schema::tests::setup_tx(&mut conn);
        assert_eq!(None, TxMapper::get(&mut tx, 1).expect("success"));
        let set_uuid = TxMapper::get_or_set_uuid_for_tx(&mut tx, 1).expect("success");
        assert_eq!(Some(set_uuid), TxMapper::get(&mut tx, 1).expect("success"));
    }

    #[test]
    fn test_bulk_setter() {
        let mut conn = schema::tests::setup_conn_bare();
        let mut tx = schema::tests::setup_tx(&mut conn);
        

        TxMapper::set_lg_mappings(&mut tx, vec![]).expect("empty map success");

        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        TxMapper::set_lg_mappings(
            &mut tx,
            vec![(1, &uuid1).into(), (2, &uuid2).into()]
        ).expect("map success");
        assert_eq!(Some(uuid1), TxMapper::get(&mut tx, 1).expect("success"));
        assert_eq!(Some(uuid2), TxMapper::get(&mut tx, 2).expect("success"));

        // Now let's replace one of the mappings.
        let new_uuid2 = Uuid::new_v4();

        TxMapper::set_lg_mappings(
            &mut tx,
            vec![(1, &uuid1).into(), (2, &new_uuid2).into()]
        ).expect("map success");
        assert_eq!(Some(uuid1), TxMapper::get(&mut tx, 1).expect("success"));
        assert_eq!(Some(new_uuid2), TxMapper::get(&mut tx, 2).expect("success"));
    }
}
