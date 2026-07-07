use anyhow::Result;
use nw_datasheet::ColumnType;

use crate::compiler::GameDataCompileUnit;
use crate::emit::GameDataCodegenFile;
use crate::naming::to_snake_ident;
use crate::typescript::source::{format_typescript_source, typescript_string_literal};

pub(super) fn emit_table_files(unit: &GameDataCompileUnit) -> Result<Vec<GameDataCodegenFile>> {
    Ok(vec![GameDataCodegenFile::new(
        "src/tables/index.ts",
        table_index_source(unit)?,
    )])
}

fn table_index_source(unit: &GameDataCompileUnit) -> Result<String> {
    let mut source = String::from(
        r#"
export type DatasheetCellKind = "string" | "number" | "boolean";

export interface ColumnDescriptor {
  readonly name: string;
  readonly fieldName: string;
  readonly crc: number;
  readonly kind: DatasheetCellKind;
  readonly rowKey: boolean;
  readonly required: boolean;
}

export interface TableDescriptor {
  readonly name: string;
  readonly nameCrc: number;
  readonly rowType: string;
  readonly rowTypeCrc: number;
  readonly rowCount: number;
  readonly sources: readonly string[];
  readonly columns: readonly ColumnDescriptor[];
}

export const TABLES = [
"#,
    );

    for table in &unit.schema_report().tables {
        source.push_str("  {\n");
        source.push_str(&format!(
            "    name: {},\n",
            typescript_string_literal(&table.table_name)
        ));
        source.push_str(&format!("    nameCrc: {},\n", table.table_name_crc));
        source.push_str(&format!(
            "    rowType: {},\n",
            typescript_string_literal(&table.row_type_name)
        ));
        source.push_str(&format!("    rowTypeCrc: {},\n", table.row_type_crc));
        source.push_str(&format!("    rowCount: {},\n", table.row_count));
        source.push_str("    sources: [");
        for (index, source_path) in table.sources.iter().enumerate() {
            if index > 0 {
                source.push_str(", ");
            }
            source.push_str(&typescript_string_literal(source_path));
        }
        source.push_str("],\n");
        source.push_str("    columns: [\n");
        for column in &table.columns {
            source.push_str("      {\n");
            source.push_str(&format!(
                "        name: {},\n",
                typescript_string_literal(&column.name)
            ));
            source.push_str(&format!(
                "        fieldName: {},\n",
                typescript_string_literal(&to_snake_ident(&column.name, "column"))
            ));
            source.push_str(&format!("        crc: {},\n", column.crc));
            source.push_str(&format!(
                "        kind: {},\n",
                typescript_string_literal(cell_kind(column.declared_type))
            ));
            source.push_str(&format!("        rowKey: {},\n", column.row_key));
            source.push_str(&format!("        required: {},\n", column.required));
            source.push_str("      },\n");
        }
        source.push_str("    ],\n");
        source.push_str("  },\n");
    }

    source.push_str(
        r#"] as const satisfies readonly TableDescriptor[];

export function tableBySourcePath(sourcePath: string): TableDescriptor | undefined {
  return TABLES.find((table) => table.sources.includes(sourcePath));
}
"#,
    );

    format_typescript_source(&source).map_err(Into::into)
}

fn cell_kind(column_type: ColumnType) -> &'static str {
    match column_type {
        ColumnType::String => "string",
        ColumnType::Number => "number",
        ColumnType::Boolean => "boolean",
    }
}
