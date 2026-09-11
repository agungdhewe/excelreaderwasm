export interface ChunkManifestItem {
  chunk_index: number;
  chunk_size: number;
  start_row: number;
  end_row: number;
  checksum: string;
}

export interface ChunkMeta {
  uploadId: string;
  chunkIndex: number;
  totalChunks: number;
  chunkSize: number;
  startRow: number;
  endRow: number;
  totalRows: number;
  isLastChunk: boolean;
  progressPercent: number;
  checksum: string;
}

export interface ProcessSummary {
  success: boolean;
  uploadId: string;
  totalRows: number;
  totalChunks: number;
  chunkSize: number;
  sheetName: string;
  headers: string[];
  totalChecksum: string;
  chunkManifest: ChunkManifestItem[];
  uploadedChunks?: number[];
  verification?: VerificationResult;
}

export interface VerificationResult {
  isComplete: boolean;
  missingChunks: number[];
  expectedRows: number;
  serverRows: number;
  expectedChunks: number;
  serverChunks: number;
  message: string;
  serverResponse?: any;
}

export interface UploadResult extends ProcessSummary {
  chunks?: any[][];
}

export interface UploadSpreadsheetOptions {
  file?: File | Blob | ArrayBuffer | Uint8Array;
  validHeader?: string | string[];
  mappingHeader?: Record<string, string> | string;
  rowChunk?: number;
  sheetIndex?: number;
  sheet_index?: number;
  headerRownum?: number;
  header_rownum?: number;
  uploadId?: string;
  wasmUrl?: string | URL;
  onUploading?: (chunk: any[], meta: ChunkMeta) => Promise<void> | void;
  onChunk?: (chunk: any[], meta: ChunkMeta) => Promise<void> | void;
  onProgress?: (meta: ChunkMeta) => void;
  onCompleted?: (summary: ProcessSummary) => Promise<any> | any;
  verifyServer?: (manifest: ProcessSummary) => Promise<any>;
}

export function initWasm(wasmSource?: string | URL | Request | WebAssembly.Module): Promise<void>;

export function verifyUploadCompleteness(manifest: ProcessSummary, serverStatus: any): VerificationResult;

export function uploadSpreadsheet(
  file: File | Blob | ArrayBuffer | Uint8Array,
  validHeader?: string | string[],
  mappingHeader?: Record<string, string> | string,
  rowChunk?: number,
  options?: Omit<UploadSpreadsheetOptions, 'file' | 'validHeader' | 'mappingHeader' | 'rowChunk'>
): Promise<UploadResult>;

export function uploadSpreadsheet(
  options: UploadSpreadsheetOptions
): Promise<UploadResult>;

export class SpreadsheetUploader extends EventTarget {
  constructor(options?: UploadSpreadsheetOptions);
  on(eventName: 'uploading', listener: (chunk: any[], meta: ChunkMeta) => void | Promise<void>): this;
  on(eventName: 'progress', listener: (meta: ChunkMeta) => void): this;
  on(eventName: 'complete', listener: (result: UploadResult) => void): this;
  on(eventName: 'error', listener: (error: any) => void): this;
  on(eventName: string, listener: (...args: any[]) => void): this;
  process(
    file: File | Blob | ArrayBuffer | Uint8Array,
    validHeader?: string | string[],
    mappingHeader?: Record<string, string> | string,
    rowChunk?: number,
    options?: UploadSpreadsheetOptions
  ): Promise<UploadResult>;
}

export class ExcelReader {
  constructor(bytes: Uint8Array);
  free(): void;
  getSheetNames(): string[];
  getHeaders(sheet_index?: number, header_rownum?: number): string[];
  validateHeaders(valid_header: string, sheet_index?: number, header_rownum?: number): { valid: boolean; headers: string[] };
  parseSpreadsheet(
    valid_header: string,
    mapping_header: string,
    row_chunk: number,
    sheet_index?: number | null,
    header_rownum?: number | null,
    callback?: Function | null
  ): any;
}

export function parseSpreadsheetDirect(
  bytes: Uint8Array,
  valid_header: string,
  mapping_header: string,
  row_chunk: number,
  sheet_index?: number | null,
  header_rownum?: number | null,
  callback?: Function | null
): any;

export default uploadSpreadsheet;
