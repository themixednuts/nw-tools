const DATASHEET_VERSIONS = [0x11, 0x12] as const;
const DATASHEET_NAME_CRC_OFFSET = 0x04;
const DATASHEET_NAME_STRING_OFFSET = 0x08;
const DATASHEET_TYPE_CRC_OFFSET = 0x0c;
const DATASHEET_TYPE_STRING_OFFSET = 0x10;
const DATASHEET_DATA_SIZE_OFFSET = 0x38;
const DATASHEET_COLUMN_COUNT_OFFSET = 0x44;
const DATASHEET_ROW_COUNT_OFFSET = 0x48;
const DATASHEET_COLUMN_RECORDS_OFFSET = 0x5c;
const DATASHEET_DATA_END_OFFSET = 0x38;
const DATASHEET_U32_SIZE = 4;
const DATASHEET_COLUMN_RECORD_SIZE = 12;
const DATASHEET_CELL_RECORD_SIZE = 8;

export enum DatasheetColumnType {
  String = 0x01,
  Number = 0x02,
  Boolean = 0x03
}

export interface Datasheet {
  readonly version: number;
  readonly nameCrc: number;
  readonly name: string;
  readonly typeCrc: number;
  readonly typeName: string;
  readonly columns: readonly DatasheetColumn[];
  readonly rows: readonly DatasheetRow[];
}

export interface DatasheetColumn {
  readonly crc: number;
  readonly name: string;
  readonly columnType: DatasheetColumnType;
}

export interface DatasheetRow {
  readonly cells: readonly DatasheetCell[];
}

export interface DatasheetCell {
  readonly crc: number;
  readonly value: DatasheetCellValue;
}

export type DatasheetCellValue = { readonly kind: "string"; readonly value: string } | { readonly kind: "number"; readonly value: number } | { readonly kind: "boolean"; readonly value: boolean };

export function parseDatasheet(bytes: Uint8Array): Datasheet {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = readU32(view, 0x00, "datasheet version");
  if (!DATASHEET_VERSIONS.includes(version as 0x11 | 0x12)) {
    throw new Error(`unsupported datasheet version 0x${version.toString(16)}`);
  }

  const nameCrc = readU32(view, DATASHEET_NAME_CRC_OFFSET, "datasheet name crc");
  const nameOffset = readU32(view, DATASHEET_NAME_STRING_OFFSET, "datasheet name string offset");
  const typeCrc = readU32(view, DATASHEET_TYPE_CRC_OFFSET, "datasheet type crc");
  const typeOffset = readU32(view, DATASHEET_TYPE_STRING_OFFSET, "datasheet type string offset");
  const dataSize = readU32(view, DATASHEET_DATA_SIZE_OFFSET, "datasheet data size");
  const columnCount = readU32(view, DATASHEET_COLUMN_COUNT_OFFSET, "datasheet column count");
  const rowCount = readU32(view, DATASHEET_ROW_COUNT_OFFSET, "datasheet row count");
  const columnsOffset = DATASHEET_COLUMN_RECORDS_OFFSET;
  const columnsLength = checkedProduct(columnCount, DATASHEET_COLUMN_RECORD_SIZE, "datasheet column records");
  const cellsOffset = columnsOffset + columnsLength;
  const cellCount = checkedProduct(rowCount, columnCount, "datasheet cell count");
  const cellsLength = checkedProduct(cellCount, DATASHEET_CELL_RECORD_SIZE, "datasheet cell records");
  const stringsOffset = cellsOffset + cellsLength;
  const expectedStringsOffset = DATASHEET_DATA_END_OFFSET + DATASHEET_U32_SIZE + dataSize;
  if (stringsOffset !== expectedStringsOffset) {
    throw new Error(`datasheet data_size points string table at 0x${expectedStringsOffset.toString(16)}, expected 0x${stringsOffset.toString(16)}`);
  }
  assertByteRange(bytes, stringsOffset, bytes.byteLength - stringsOffset, "datasheet string table");

  const stringAt = (offset: number, label: string): string => readString(bytes, stringsOffset, offset, label);
  const columns: DatasheetColumn[] = [];
  for (let columnIndex = 0; columnIndex < columnCount; columnIndex += 1) {
    const offset = columnsOffset + columnIndex * DATASHEET_COLUMN_RECORD_SIZE;
    assertByteRange(bytes, offset, DATASHEET_COLUMN_RECORD_SIZE, `datasheet column ${columnIndex}`);
    const rawColumnType = readU32(view, offset + 8, `datasheet column ${columnIndex} type`);
    const columnType = parseColumnType(rawColumnType, columnIndex);
    columns.push({
      crc: readU32(view, offset, `datasheet column ${columnIndex} crc`),
      name: stringAt(readI32(view, offset + 4, `datasheet column ${columnIndex} name offset`), `datasheet column ${columnIndex} name`),
      columnType
    });
  }

  const rows: DatasheetRow[] = [];
  for (let rowIndex = 0; rowIndex < rowCount; rowIndex += 1) {
    const cells: DatasheetCell[] = [];
    for (let columnIndex = 0; columnIndex < columnCount; columnIndex += 1) {
      const record = cellsOffset + (rowIndex * columnCount + columnIndex) * DATASHEET_CELL_RECORD_SIZE;
      assertByteRange(bytes, record, DATASHEET_CELL_RECORD_SIZE, `datasheet row ${rowIndex} column ${columnIndex}`);
      cells.push({
        crc: readU32(view, record, `datasheet row ${rowIndex} column ${columnIndex} crc`),
        value: parseCellValue(view, stringAt, record + 4, columns[columnIndex] as DatasheetColumn, rowIndex)
      });
    }
    rows.push({ cells });
  }

  return {
    version,
    nameCrc,
    name: stringAt(nameOffset, "datasheet name"),
    typeCrc,
    typeName: stringAt(typeOffset, "datasheet type name"),
    columns,
    rows
  };
}

function parseColumnType(raw: number, columnIndex: number): DatasheetColumnType {
  switch (raw) {
    case DatasheetColumnType.String:
      return DatasheetColumnType.String;
    case DatasheetColumnType.Number:
      return DatasheetColumnType.Number;
    case DatasheetColumnType.Boolean:
      return DatasheetColumnType.Boolean;
    default:
      throw new Error(`unknown datasheet column type ${raw} at column ${columnIndex}`);
  }
}

function parseCellValue(view: DataView, stringAt: (offset: number, label: string) => string, valueOffset: number, column: DatasheetColumn, rowIndex: number): DatasheetCellValue {
  switch (column.columnType) {
    case DatasheetColumnType.String:
      return {
        kind: "string",
        value: stringAt(readU32(view, valueOffset, `${column.name} string offset`), `row ${rowIndex} ${column.name}`)
      };
    case DatasheetColumnType.Number:
      return { kind: "number", value: view.getFloat32(valueOffset, true) };
    case DatasheetColumnType.Boolean:
      return { kind: "boolean", value: view.getInt32(valueOffset, true) !== 0 };
  }
}

function readU32(view: DataView, offset: number, label: string): number {
  assertViewRange(view, offset, 4, label);
  return view.getUint32(offset, true);
}

function readI32(view: DataView, offset: number, label: string): number {
  assertViewRange(view, offset, 4, label);
  return view.getInt32(offset, true);
}

function readString(bytes: Uint8Array, stringsOffset: number, offset: number, label: string): string {
  if (offset < 0) {
    throw new Error(`${label} has negative string offset ${offset}`);
  }
  const start = stringsOffset + offset;
  assertByteRange(bytes, start, 1, label);
  let end = start;
  while (end < bytes.byteLength && bytes[end] !== 0) {
    end += 1;
  }
  if (end === bytes.byteLength) {
    throw new Error(`${label} is unterminated`);
  }
  return new TextDecoder().decode(bytes.subarray(start, end));
}

function checkedProduct(left: number, right: number, label: string): number {
  const product = left * right;
  if (!Number.isSafeInteger(product)) {
    throw new Error(`${label} length is not a safe integer`);
  }
  return product;
}

function assertViewRange(view: DataView, offset: number, length: number, label: string): void {
  assertLength(view.byteLength, offset, length, label);
}

function assertByteRange(bytes: Uint8Array, offset: number, length: number, label: string): void {
  assertLength(bytes.byteLength, offset, length, label);
}

function assertLength(byteLength: number, offset: number, length: number, label: string): void {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) || offset < 0 || length < 0 || offset + length > byteLength) {
    throw new Error(`${label} out of bounds`);
  }
}
