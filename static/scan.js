// Barcode scanner for /scan. Uses the bundled ZXing build (no CDN). Kept in its
// own file so the Content-Security-Policy can forbid inline scripts.
(function () {
  var box = document.getElementById('scanner');
  if (!box || !window.ZXing) return;
  var mode = box.getAttribute('data-mode') === 'use' ? 'use' : 'add';
  var startBtn = document.getElementById('start'), video = document.getElementById('video'),
      hint = document.getElementById('hint'), err = document.getElementById('err'),
      torchBtn = document.getElementById('torch');
  var F = ZXing.BarcodeFormat;
  var hints = new Map();
  hints.set(ZXing.DecodeHintType.POSSIBLE_FORMATS,
            [F.EAN_13, F.EAN_8, F.UPC_A, F.UPC_E, F.CODE_128, F.CODE_39, F.ITF, F.QR_CODE]);
  hints.set(ZXing.DecodeHintType.TRY_HARDER, true);
  var reader = new ZXing.BrowserMultiFormatReader(hints, 200);
  var STRONG = [F.EAN_13, F.EAN_8, F.UPC_A, F.UPC_E];
  var done = false, audio = null, last = null;
  startBtn.hidden = false;

  function beep() {
    try {
      var o = audio.createOscillator(), g = audio.createGain();
      o.frequency.value = 1400; g.gain.value = 0.08;
      o.connect(g); g.connect(audio.destination); o.start(); o.stop(audio.currentTime + 0.09);
    } catch (e) {}
  }

  // Shop barcodes (EAN/UPC) carry a check digit, so one read is enough. Other
  // formats misread more often: wait until the same code is read twice in a row.
  function onResult(result) {
    if (!result) return;
    var text = result.getText();
    if (STRONG.indexOf(result.getBarcodeFormat()) === -1 && text !== last) { last = text; return; }
    found(text);
  }

  function found(text) {
    if (done) return;
    done = true;
    if (navigator.vibrate) navigator.vibrate(60);
    if (audio) beep();
    hint.textContent = (box.dataset.found || 'Found') + ' ' + text + ' …';
    reader.reset();
    location.href = '/scan/code?' + new URLSearchParams({ code: text, mode: mode });
  }

  function setupTorch() {
    try {
      var track = video.srcObject && video.srcObject.getVideoTracks()[0];
      var caps = track && track.getCapabilities ? track.getCapabilities() : {};
      if (!caps.torch) return;
      torchBtn.hidden = false;
      torchBtn.addEventListener('click', function () {
        var on = torchBtn.getAttribute('aria-pressed') !== 'true';
        track.applyConstraints({ advanced: [{ torch: on }] }).then(function () {
          torchBtn.setAttribute('aria-pressed', on ? 'true' : 'false');
        }).catch(function () { torchBtn.hidden = true; });
      });
    } catch (e) {}
  }

  function start() {
    err.hidden = true;
    if (!window.isSecureContext || !navigator.mediaDevices) {
      err.textContent = box.dataset.https;
      err.hidden = false;
      return;
    }
    box.hidden = false;
    startBtn.hidden = true;
    reader.decodeFromConstraints(
      { video: { facingMode: { ideal: 'environment' }, width: { ideal: 1280 }, height: { ideal: 720 } } },
      video, onResult
    ).then(setupTorch).catch(function (e) {
      box.hidden = true; startBtn.hidden = false;
      err.textContent = box.dataset.nocam + ' (' + (e && e.name || e) + ')';
      err.hidden = false;
    });
  }

  startBtn.addEventListener('click', function () {
    try { audio = audio || new (window.AudioContext || window.webkitAudioContext)(); } catch (e) {}
    try { sessionStorage.setItem('vorrat-cam', '1'); } catch (e) {}
    start();
  });
  // iOS grants the camera only after a tap, but remember the choice for next time.
  try { if (sessionStorage.getItem('vorrat-cam') === '1') start(); } catch (e) {}
})();
