export async function copyAddress(address, clipboard = globalThis.navigator?.clipboard) {
  if (!address) return false;
  if (!clipboard?.writeText) throw new Error('Clipboard unavailable');
  await clipboard.writeText(address);
  return true;
}

export function renderSupport(details, document) {
  const paypal = document.getElementById('paypal');
  if (details.paypal_url) {
    paypal.href = details.paypal_url;
    paypal.removeAttribute('aria-disabled');
    document.getElementById('paypal-hint').textContent = '';
  }
  for (const [id, name] of [['bitcoin', 'Bitcoin'], ['monero', 'Monero'], ['dogecoin', 'Dogecoin']]) {
    const button = document.getElementById(id);
    const address = details[id] || '';
    button.querySelector('code').textContent = address;
    button.disabled = !address;
    if (!address) continue;
    button.parentElement.querySelector('.hint').textContent = '';
    button.addEventListener('click', async () => {
      const feedback = document.getElementById('feedback');
      try {
        await copyAddress(address);
        feedback.textContent = `${name} address copied.`;
      } catch {
        feedback.textContent = 'Could not copy. Select the address and copy it manually.';
      }
    });
  }
}

if (typeof document !== 'undefined') {
  fetch('details.json', {cache: 'no-cache'})
    .then(response => {if (!response.ok) throw new Error('Support details unavailable'); return response.json();})
    .then(details => renderSupport(details, document))
    .catch(() => {document.getElementById('feedback').textContent = 'Support details are temporarily unavailable.';});
}
