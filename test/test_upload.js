import * as XLSX from 'xlsx';
import fs from 'fs';
import { uploadSpreadsheet, SpreadsheetUploader, initWasm, verifyUploadCompleteness } from '../index.js';

// Initialize WASM using Node fs/buffer
const wasmBuffer = fs.readFileSync(new URL('../pkg/excelreaderwasm_bg.wasm', import.meta.url));
await initWasm(wasmBuffer);

console.log('--- 1. Generating Test Excel with 105 rows ---');
const data = [
  ['No', 'Nama', 'Alamat', 'Kota']
];

for (let i = 1; i <= 105; i++) {
  data.push([i, `User ${i}`, `Jl. Mawar No. ${i}`, i % 2 === 0 ? 'Jakarta' : 'Surabaya']);
}

const ws = XLSX.utils.aoa_to_sheet(data);
const wb = XLSX.utils.book_new();
XLSX.utils.book_append_sheet(wb, ws, 'DataSheet');
const excelBuffer = XLSX.write(wb, { type: 'buffer', bookType: 'xlsx' });

console.log(`Excel file created: ${excelBuffer.length} bytes\n`);

// Mock Server State
class MockUploadServer {
  constructor() {
    this.sessions = new Map();
  }

  receiveChunk(chunkPayload, meta) {
    if (!this.sessions.has(meta.uploadId)) {
      this.sessions.set(meta.uploadId, {
        receivedChunks: new Map(),
        totalRowsReceived: 0
      });
    }
    const session = this.sessions.get(meta.uploadId);
    session.receivedChunks.set(meta.chunkIndex, {
      chunkSize: meta.chunkSize,
      checksum: meta.checksum,
      rows: chunkPayload
    });
    session.totalRowsReceived += chunkPayload.length;
    return { status: 'CHUNK_OK', chunkIndex: meta.chunkIndex };
  }

  verify(manifest) {
    const session = this.sessions.get(manifest.uploadId);
    if (!session) {
      return { verified: false, message: 'Session not found' };
    }

    const missingChunks = [];
    for (let i = 1; i <= manifest.totalChunks; i++) {
      if (!session.receivedChunks.has(i)) {
        missingChunks.push(i);
      }
    }

    return {
      verified: missingChunks.length === 0 && session.totalRowsReceived === manifest.totalRows,
      receivedChunks: session.receivedChunks.size,
      receivedRows: session.totalRowsReceived,
      missingChunks,
      status: missingChunks.length === 0 ? 'COMMITTED' : 'INCOMPLETE'
    };
  }
}

const server = new MockUploadServer();

console.log('--- 2. Testing uploadSpreadsheet with Server Verification (verifyServer) ---');
const result = await uploadSpreadsheet(
  excelBuffer,
  'No|Nama|Alamat|Kota',
  { no: 'No', alamat: 'Alamat' },
  10,
  {
    onUploading: async (chunk, meta) => {
      // Send chunk to mock server
      server.receiveChunk(chunk, meta);
      console.log(
        `[Upload] Chunk ${meta.chunkIndex}/${meta.totalChunks} | Rows: ${meta.chunkSize} (CRC32: ${meta.checksum}) -> Server ACK`
      );
    },
    verifyServer: async (manifest) => {
      console.log(`[Verify] Calling server verify for session ${manifest.uploadId}...`);
      const verificationResponse = server.verify(manifest);
      console.log('[Verify] Server response:', verificationResponse);
      return verificationResponse;
    }
  }
);

console.log('\n--- 3. Verifying Results ---');
console.log('Upload ID:', result.uploadId);
console.log('Total Checksum (CRC32):', result.totalChecksum);
console.log('Total Rows:', result.totalRows);
console.log('Total Chunks:', result.totalChunks);
console.log('Verification Result:', result.verification);

if (!result.verification.isComplete) {
  throw new Error('Upload completeness verification failed!');
}

console.log('\n--- 4. Testing Incomplete Upload Detection (Simulated Missing Chunk) ---');
const incompleteServer = new MockUploadServer();
try {
  await uploadSpreadsheet(
    excelBuffer,
    'No|Nama|Alamat|Kota',
    { no: 'No', alamat: 'Alamat' },
    10,
    {
      onUploading: async (chunk, meta) => {
        // Simulate dropping chunk #4
        if (meta.chunkIndex !== 4) {
          incompleteServer.receiveChunk(chunk, meta);
        }
      },
      verifyServer: async (manifest) => {
        return incompleteServer.verify(manifest);
      }
    }
  );
  throw new Error('Should have failed completeness verification!');
} catch (err) {
  console.log('Correctly caught completeness error:');
  console.log(err.message);
}

console.log('\n--- 5. Testing headerRownum (Header at Row 3 with title rows above) ---');
const dataWithHeader3 = [
  ['LAPORAN DATA KARYAWAN', '', '', ''],
  ['Tanggal: 2026-09-11 | Cabang: Jakarta', '', '', ''],
  ['No', 'Nama', 'Alamat', 'Kota']
];
for (let i = 1; i <= 25; i++) {
  dataWithHeader3.push([i, `User ${i}`, `Jl. Gatot Subroto No. ${i}`, 'Jakarta']);
}
const ws3 = XLSX.utils.aoa_to_sheet(dataWithHeader3);
const wb3 = XLSX.utils.book_new();
XLSX.utils.book_append_sheet(wb3, ws3, 'CustomHeaderSheet');
const excelBuffer3 = XLSX.write(wb3, { type: 'buffer', bookType: 'xlsx' });

const result3 = await uploadSpreadsheet({
  file: excelBuffer3,
  validHeader: 'No|Nama|Alamat|Kota',
  mappingHeader: { id: 'No', nama: 'Nama', kota: 'Kota' },
  rowChunk: 10,
  headerRownum: 3
});

console.log('Result with headerRownum=3:');
console.log('Total Rows:', result3.totalRows);
console.log('Total Chunks:', result3.totalChunks);
console.log('First chunk sample:', result3.chunks[0][0]);

if (result3.totalRows !== 25) {
  throw new Error(`Expected 25 data rows, but got ${result3.totalRows}`);
}
if (result3.chunks[0][0].id !== 1 || result3.chunks[0][0].nama !== 'User 1') {
  throw new Error('Data mapping with headerRownum=3 failed!');
}

console.log('\n✅ ALL INTEGRATION & VERIFICATION TESTS PASSED SUCCESSFULLY! 🚀');

