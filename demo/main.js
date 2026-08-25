import { uploadSpreadsheet, SpreadsheetUploader, verifyUploadCompleteness } from '../index.js';

let selectedFile = null;

const fileInput = document.getElementById('fileInput');
const dropzone = document.getElementById('dropzone');
const dropzoneText = document.getElementById('dropzone-text');
const btnUpload = document.getElementById('btnUpload');
const btnGenSample = document.getElementById('btnGenSample');
const btnGenBig = document.getElementById('btnGenBig');
const validHeaderInput = document.getElementById('validHeader');
const mappingHeaderInput = document.getElementById('mappingHeader');
const rowChunkInput = document.getElementById('rowChunk');

const progressSection = document.getElementById('progressSection');
const progressBar = document.getElementById('progressBar');
const progressStatus = document.getElementById('progressStatus');
const progressPercent = document.getElementById('progressPercent');
const chunkStatus = document.getElementById('chunkStatus');
const rowsStatus = document.getElementById('rowsStatus');
const logBox = document.getElementById('logBox');

const previewCard = document.getElementById('previewCard');
const previewThead = document.getElementById('previewThead');
const previewTbody = document.getElementById('previewTbody');

function log(msg, type = 'info') {
  const div = document.createElement('div');
  div.className = `log-entry ${type}`;
  const time = new Date().toLocaleTimeString();
  div.textContent = `[${time}] ${msg}`;
  logBox.appendChild(div);
  logBox.scrollTop = logBox.scrollHeight;
}

function setFile(file, name) {
  selectedFile = file;
  dropzoneText.innerHTML = `<strong>File terpilih:</strong> ${name || file.name} (${(file.size || file.byteLength).toLocaleString()} bytes)`;
  btnUpload.disabled = false;
  log(`File siap: ${name || file.name}`, 'info');
}

dropzone.addEventListener('click', () => fileInput.click());
dropzone.addEventListener('dragover', (e) => {
  e.preventDefault();
  dropzone.classList.add('dragover');
});
dropzone.addEventListener('dragleave', () => dropzone.classList.remove('dragover'));
dropzone.addEventListener('drop', (e) => {
  e.preventDefault();
  dropzone.classList.remove('dragover');
  if (e.dataTransfer.files.length) {
    setFile(e.dataTransfer.files[0]);
  }
});
fileInput.addEventListener('change', () => {
  if (fileInput.files.length) {
    setFile(fileInput.files[0]);
  }
});

function generateExcel(rowCount, filename) {
  log(`Membuat file Excel sample dengan ${rowCount.toLocaleString()} baris...`, 'info');
  const data = [['No', 'Nama', 'Alamat', 'Kota']];
  const cities = ['Jakarta', 'Surabaya', 'Bandung', 'Medan', 'Semarang', 'Yogyakarta', 'Makassar', 'Bali'];
  
  for (let i = 1; i <= rowCount; i++) {
    data.push([
      i,
      `User ${i}`,
      `Jl. Jendral Sudirman Kav. ${i}`,
      cities[i % cities.length]
    ]);
  }

  const ws = XLSX.utils.aoa_to_sheet(data);
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, ws, 'DataSheet');
  const buffer = XLSX.write(wb, { type: 'array', bookType: 'xlsx' });
  const blob = new Blob([buffer], { type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' });
  blob.name = filename;
  setFile(blob, filename);
  log(`Sample Excel dibuat (${buffer.byteLength.toLocaleString()} bytes). Klik tombol Mulai Upload!`, 'success');
}

btnGenSample.addEventListener('click', () => {
  generateExcel(105, 'sample_105_rows.xlsx');
});

btnGenBig.addEventListener('click', () => {
  generateExcel(10000, 'sample_10000_rows.xlsx');
});

// Mock Server State untuk demonstrasi verifikasi
class DemoServer {
  constructor() {
    this.sessions = new Map();
  }

  receiveChunk(uploadId, chunkPayload, meta) {
    if (!this.sessions.has(uploadId)) {
      this.sessions.set(uploadId, {
        chunks: new Map(),
        totalRows: 0
      });
    }
    const session = this.sessions.get(uploadId);
    session.chunks.set(meta.chunkIndex, {
      size: chunkPayload.length,
      checksum: meta.checksum
    });
    session.totalRows += chunkPayload.length;
    return { ok: true };
  }

  verify(manifest) {
    const session = this.sessions.get(manifest.uploadId);
    if (!session) return { verified: false, message: 'Session not found' };

    const missingChunks = [];
    for (let i = 1; i <= manifest.totalChunks; i++) {
      if (!session.chunks.has(i)) {
        missingChunks.push(i);
      }
    }

    const isComplete = (missingChunks.length === 0) && (session.totalRows === manifest.totalRows);
    return {
      verified: isComplete,
      receivedChunks: session.chunks.size,
      receivedRows: session.totalRows,
      missingChunks,
      status: isComplete ? 'COMMITTED' : 'INCOMPLETE'
    };
  }
}

const mockServer = new DemoServer();

btnUpload.addEventListener('click', async () => {
  if (!selectedFile) return;

  btnUpload.disabled = true;
  btnGenSample.disabled = true;
  btnGenBig.disabled = true;
  progressSection.style.display = 'block';
  previewCard.style.display = 'none';
  previewTbody.innerHTML = '';
  previewThead.innerHTML = '';

  const validHeader = validHeaderInput.value;
  const mappingHeader = mappingHeaderInput.value;
  const rowChunk = parseInt(rowChunkInput.value, 10) || 10;

  log(`Memulai uploadSpreadsheet (rowChunk: ${rowChunk})...`, 'info');

  const startTime = performance.now();
  let collectedRows = [];

  try {
    const result = await uploadSpreadsheet(selectedFile, validHeader, mappingHeader, rowChunk, {
      onUploading: async (chunk, meta) => {
        // 1. Kirim chunk ke server
        mockServer.receiveChunk(meta.uploadId, chunk, meta);

        // 2. Update UI
        progressBar.style.width = `${meta.progressPercent}%`;
        progressPercent.textContent = `${meta.progressPercent}%`;
        chunkStatus.textContent = `Chunk ${meta.chunkIndex} / ${meta.totalChunks}`;
        rowsStatus.textContent = `${meta.endRow} / ${meta.totalRows} Baris`;
        progressStatus.textContent = `Mengupload chunk ${meta.chunkIndex}...`;

        log(
          `[onUploading] Chunk ${meta.chunkIndex}/${meta.totalChunks} (${meta.startRow}-${meta.endRow} dari ${meta.totalRows} baris) [CRC32: ${meta.checksum}] -> ACK`,
          'info'
        );

        collectedRows.push(...chunk);

        // Simulasi delay transfer network
        await new Promise(r => setTimeout(r, 20));
      },

      // 3. Verifikasi kelengkapan seluruh data di server saat semua chunk selesai
      verifyServer: async (manifest) => {
        log(`[verifyServer] Mengirim manifest verifikasi ke server (Session: ${manifest.uploadId})...`, 'warn');
        const serverStatus = mockServer.verify(manifest);
        log(`[verifyServer] Hasil Verifikasi Server: ${JSON.stringify(serverStatus)}`, 'success');
        return serverStatus;
      }
    });

    const elapsed = (performance.now() - startTime).toFixed(1);
    progressStatus.textContent = 'Verifikasi Sukses & Selesai!';
    log(`✅ SELURUH DATA TERUPLOAD SEMPURNA KE SERVER! Total ${result.totalRows} baris dalam ${result.totalChunks} chunk (CRC32: ${result.totalChecksum}) selesai dalam ${elapsed} ms.`, 'success');

    // Display preview table
    if (collectedRows.length > 0) {
      previewCard.style.display = 'block';
      const keys = Object.keys(collectedRows[0]);
      
      const trHead = document.createElement('tr');
      keys.forEach(k => {
        const th = document.createElement('th');
        th.textContent = k;
        trHead.appendChild(th);
      });
      previewThead.appendChild(trHead);

      const displayRows = collectedRows.slice(0, 25);
      displayRows.forEach(row => {
        const tr = document.createElement('tr');
        keys.forEach(k => {
          const td = document.createElement('td');
          td.textContent = row[k] !== undefined && row[k] !== null ? row[k] : '';
          tr.appendChild(td);
        });
        previewTbody.appendChild(tr);
      });

      if (collectedRows.length > 25) {
        const tr = document.createElement('tr');
        const td = document.createElement('td');
        td.colSpan = keys.length;
        td.style.textAlign = 'center';
        td.style.fontStyle = 'italic';
        td.textContent = `... dan ${collectedRows.length - 25} baris lainnya (hanya menampilkan 25 baris pertama) ...`;
        tr.appendChild(td);
        previewTbody.appendChild(tr);
      }
    }
  } catch (err) {
    log(`❌ Error: ${err.message || err}`, 'error');
    progressStatus.textContent = 'Gagal';
  } finally {
    btnUpload.disabled = false;
    btnGenSample.disabled = false;
    btnGenBig.disabled = false;
  }
});
