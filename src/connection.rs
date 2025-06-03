use std::error::Error;

use crate::{
    config::DATABASE_PATH,
    value::{Value, ValueType},
};
use rusqlite::{Connection as RsqConnection, OpenFlags, Params, types::Value as RsqValue};

/// A table of Values, generated through a query to some database
#[derive(Debug, Clone)]
pub struct Table {
    pub(crate) rows: Vec<Vec<Value>>,
    pub(crate) columns: Vec<String>,
    pub(crate) query: Option<String>,
}

impl Table {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|column| column.as_str() == name)
    }

    /// Function to get the value stored at the column with the
    /// specified name within the row at the passed index
    pub fn row_get(&self, row: usize, name: &str) -> Option<&Value> {
        let col = self.column_index(name)?;
        Some(&self.rows[row][col])
    }
}

#[derive(Debug)]
pub struct ColumnInfo {
    pub(crate) name: String,
    pub(crate) data_type: ValueType,
    pub(crate) is_not_null: bool,
    pub(crate) default: Value,
    pub(crate) is_primary_key: bool,
    pub(crate) cid: usize,
}

impl std::fmt::Display for ColumnInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let info_vec: Vec<&str> = [
            self.is_primary_key.then_some("PK"),
            self.is_not_null.then_some("Required"),
            Some(match self.data_type {
                ValueType::Null => "Null",
                ValueType::Integer => "Int",
                ValueType::Real => "Real",
                ValueType::Text => "Text",
                ValueType::Blob => "Blob",
            }),
        ]
        .into_iter()
        .flatten()
        .collect();
        write!(f, "{}", info_vec.join(", "))
    }
}

/// A connection to the database updated and read by the app
pub struct Connection {
    connection: RsqConnection,
}

impl Connection {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let connection = RsqConnection::open_with_flags(
            DATABASE_PATH,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(Self { connection })
    }

    /// Computes the passed query using this connection
    ///
    /// returns a Result containing the resulting rows of the table,
    /// or an Error indicating the failure
    pub fn query<T: Params>(&self, query: &str, params: T) -> Result<Table, Box<dyn Error>> {
        // generate a unique, index associated pair of vectors for
        // the column names and the row data associated with those columns
        let mut stmt = self.connection.prepare(query)?;
        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|col| col.to_string())
            .collect();
        // map the query into a 2d array of returned values
        let rows: Vec<Vec<Value>> = stmt
            .query_map(params, |row| {
                let mut row_fields: Vec<Value> = Vec::new();
                let mut ind = 0;
                // turbofish needed to ensure proper typing
                while let Ok(field) = row.get::<usize, RsqValue>(ind) {
                    row_fields.push(field.into());
                    ind += 1;
                }
                Ok(row_fields)
            })?
            .filter_map(|res| res.ok())
            .collect();
        let query = stmt.expanded_sql();
        Ok(Table {
            rows,
            columns,
            query,
        })
    }

    /// Simple wrapper over Rusqlite's Statement.insert(params) function
    /// which should be only used for the sake of a single insertion
    /// An example insert statement is as follows:
    ///
    /// `INSERT INTO table (col1, col2, col3) VALUES (val1, val2, val3);`
    pub fn insert<T: Params>(&self, query: &str, params: T) -> Result<i64, Box<dyn Error>> {
        let mut stmt = self.connection.prepare(query)?;
        Ok(stmt.insert(params)?)
    }

    /// Simple wrapper over Rusqlite's Statement.execute(params) function
    /// which should be only used for the sake of deletion.
    /// An example delete statement is as follows:
    ///
    /// `DELETE FROM table WHERE col_name = value ORDER BY col LIMIT num;`
    pub fn delete<T: Params>(&self, query: &str, params: T) -> Result<usize, Box<dyn Error>> {
        let mut stmt = self.connection.prepare(query)?;
        Ok(stmt.execute(params)?)
    }

    /// Simple wrapper over Rusqlite's Statement.execute(params) function
    /// which should be only used for the sake of modifying a cell.
    /// An example modification statement is as follows:
    ///
    /// `UPDATE table SET col_name = value WHERE pk_name = pk_val;`
    pub fn modify<T: Params>(&self, query: &str, params: T) -> Result<(), Box<dyn Error>> {
        let mut stmt = self.connection.prepare(query)?;
        stmt.execute(params)?;
        Ok(())
    }

    pub fn get_columns(&self, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let stmt = self
            .connection
            .prepare(format!("SELECT * FROM {};", table).as_str())?;
        Ok(stmt
            .column_names()
            .iter()
            .map(|col| col.to_string())
            .collect())
    }

    pub fn get_column_info(&self, table: &str) -> Result<Vec<ColumnInfo>, Box<dyn Error>> {
        let mut stmt = self
            .connection
            .prepare(format!("SELECT * FROM pragma_table_info('{}');", table).as_str())?;
        let column_info = stmt
            .query_map([], |row| {
                Ok(ColumnInfo {
                    name: row.get("name")?,
                    data_type: ValueType::try_from(row.get::<&str, String>("type")?)
                        .expect("Retrieved impossible Data Type"),
                    is_not_null: row.get("notnull")?,
                    default: row.get::<&str, RsqValue>("dflt_value")?.into(),
                    is_primary_key: row.get::<&str, usize>("pk")? != 0,
                    cid: row.get("cid")?,
                })
            })?
            .filter_map(|res| res.ok())
            .collect();
        Ok(column_info)
    }
}
