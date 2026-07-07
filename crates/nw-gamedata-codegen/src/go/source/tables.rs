use anyhow::Result;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::go::source::format_go_source;
use crate::naming::to_snake_ident;

pub(super) fn emit_table_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    Ok(vec![GameDataCodegenFile::new(
        "tables/tables.go",
        table_source(unit)?,
    )])
}

fn table_source(unit: &GameDataCompileUnit) -> Result<String> {
    let mut source = String::from(
        r#"
package tables

type DatasheetCellKind string

const (
	DatasheetCellString  DatasheetCellKind = "string"
	DatasheetCellNumber  DatasheetCellKind = "number"
	DatasheetCellBoolean DatasheetCellKind = "boolean"
)

type ColumnDescriptor struct {
	Name      string
	FieldName string
	CRC       uint32
	Kind      DatasheetCellKind
	RowKey    bool
	Required  bool
}

type TableDescriptor struct {
	Name       string
	NameCRC    uint32
	RowType    string
	RowTypeCRC uint32
	RowCount   int
	Sources    []string
	Columns    []ColumnDescriptor
}

var Tables = []TableDescriptor{
"#,
    );

    for table in &unit.schema_report().tables {
        source.push_str("\t{\n");
        source.push_str(&format!("\t\tName: {},\n", go_string(&table.table_name)));
        source.push_str(&format!("\t\tNameCRC: {},\n", table.table_name_crc));
        source.push_str(&format!(
            "\t\tRowType: {},\n",
            go_string(&table.row_type_name)
        ));
        source.push_str(&format!("\t\tRowTypeCRC: {},\n", table.row_type_crc));
        source.push_str(&format!("\t\tRowCount: {},\n", table.row_count));
        source.push_str("\t\tSources: []string{");
        for (index, source_path) in table.sources.iter().enumerate() {
            if index > 0 {
                source.push_str(", ");
            }
            source.push_str(&go_string(source_path));
        }
        source.push_str("},\n");
        source.push_str("\t\tColumns: []ColumnDescriptor{\n");
        for column in &table.columns {
            source.push_str("\t\t\t{\n");
            source.push_str(&format!("\t\t\t\tName: {},\n", go_string(&column.name)));
            source.push_str(&format!(
                "\t\t\t\tFieldName: {},\n",
                go_string(&to_snake_ident(&column.name, "column"))
            ));
            source.push_str(&format!("\t\t\t\tCRC: {},\n", column.crc));
            source.push_str(&format!(
                "\t\t\t\tKind: {},\n",
                cell_kind(column.declared_type)
            ));
            source.push_str(&format!("\t\t\t\tRowKey: {},\n", column.row_key));
            source.push_str(&format!("\t\t\t\tRequired: {},\n", column.required));
            source.push_str("\t\t\t},\n");
        }
        source.push_str("\t\t},\n");
        source.push_str("\t},\n");
    }

    source.push_str(
        r#"}

func TableBySourcePath(sourcePath string) *TableDescriptor {
	for i := range Tables {
		for _, candidate := range Tables[i].Sources {
			if candidate == sourcePath {
				return &Tables[i]
			}
		}
	}
	return nil
}
"#,
    );

    format_go_source(&source).map_err(Into::into)
}

fn cell_kind(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "DatasheetCellString",
        ColumnType::Number => "DatasheetCellNumber",
        ColumnType::Boolean => "DatasheetCellBoolean",
    }
}

fn go_string(value: &str) -> String {
    format!("{value:?}")
}
