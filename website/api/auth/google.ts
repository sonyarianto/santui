const CLIENT_ID = process.env.GOOGLE_CLIENT_ID ?? '';
const CLIENT_SECRET = process.env.GOOGLE_CLIENT_SECRET ?? '';
const BASE = 'https://santuiapp.vercel.app';

function escapeHtml(s: string): string {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

function page(title: string, body: string, redirect: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <script src="https://cdn.tailwindcss.com"></script>
  <title>Santui — ${title}</title>
</head>
<body class="bg-gradient-to-br from-gray-900 via-slate-800 to-gray-900 min-h-screen flex items-center justify-center font-sans">
  <div class="bg-white/10 backdrop-blur-lg rounded-2xl shadow-2xl border border-white/20 p-8 max-w-md w-full mx-4 text-center">
    ${body}
  </div>
  <script>
    setTimeout(() => { window.location.href = ${JSON.stringify(redirect)}; }, 1500);
  </script>
</body>
</html>`;
}

export async function GET(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const code = url.searchParams.get('code');
  const stateRaw = url.searchParams.get('state');

  let port = '9842';
  if (stateRaw) {
    try {
      const state = JSON.parse(atob(stateRaw));
      if (state.port) port = String(state.port);
    } catch { /* ignore malformed state */ }
  }

  const redirectUri = `${BASE}/api/auth/google`;

  if (!CLIENT_ID || !CLIENT_SECRET) {
    return new Response(
      page(
        'Not Configured',
        `<div class="text-red-400 mb-4">
          <svg class="w-16 h-16 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"/></svg>
          <h1 class="text-2xl font-bold mb-2">Server Not Configured</h1>
        </div>
        <p class="text-gray-300 text-sm">Set <code class="text-yellow-400">GOOGLE_CLIENT_ID</code> and <code class="text-yellow-400">GOOGLE_CLIENT_SECRET</code> on Vercel.</p>`,
        `http://127.0.0.1:${port}/callback?error=server_not_configured`,
      ),
      { status: 200, headers: { 'Content-Type': 'text/html' } },
    );
  }

  // Step 1: No code yet — redirect to Google consent
  if (!code) {
    const state = btoa(JSON.stringify({ port: Number(port) }));
    const authUrl = new URL('https://accounts.google.com/o/oauth2/v2/auth');
    authUrl.searchParams.set('client_id', CLIENT_ID);
    authUrl.searchParams.set('redirect_uri', redirectUri);
    authUrl.searchParams.set('response_type', 'code');
    authUrl.searchParams.set('scope', 'openid email profile');
    authUrl.searchParams.set('state', state);
    return Response.redirect(authUrl.toString(), 302);
  }

  // Step 2: Exchange code for tokens
  let tokens: Record<string, unknown>;
  try {
    const tokenResp = await fetch('https://oauth2.googleapis.com/token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        code,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET,
        redirect_uri: redirectUri,
        grant_type: 'authorization_code',
      }),
    });
    tokens = await tokenResp.json();
  } catch (err) {
    return new Response(
      page(
        'Token Exchange Failed',
        `<div class="text-red-400 mb-4">
          <svg class="w-16 h-16 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"/></svg>
          <h1 class="text-2xl font-bold mb-2">Sign In Failed</h1>
        </div>
        <p class="text-gray-300 text-sm">Could not complete the sign-in process. Please try again.</p>`,
        `http://127.0.0.1:${port}/callback?error=token_exchange_failed`,
      ),
      { status: 200, headers: { 'Content-Type': 'text/html' } },
    );
  }

  const accessToken = tokens.access_token;
  if (!accessToken || typeof accessToken !== 'string') {
    return new Response(
      page(
        'No Access Token',
        `<div class="text-red-400 mb-4">
          <svg class="w-16 h-16 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"/></svg>
          <h1 class="text-2xl font-bold mb-2">Sign In Failed</h1>
        </div>
        <p class="text-gray-300 text-sm">No access token was returned by Google. Please try again.</p>`,
        `http://127.0.0.1:${port}/callback?error=no_access_token`,
      ),
      { status: 200, headers: { 'Content-Type': 'text/html' } },
    );
  }

  // Step 3: Fetch user info
  let user: Record<string, unknown> = {};
  try {
    const userResp = await fetch('https://www.googleapis.com/oauth2/v2/userinfo', {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    user = await userResp.json();
  } catch { /* proceed with partial data */ }

  const params = new URLSearchParams({
    access_token: accessToken,
    provider: 'google',
    id: String(user.id ?? ''),
    email: String(user.email ?? ''),
    name: String(user.name ?? ''),
  });
  if (user.picture) params.set('avatar_url', String(user.picture));

  const localUrl = `http://127.0.0.1:${port}/callback?${params}`;

  const avatar = String(user.picture ?? '');
  const userName = String(user.name ?? user.email ?? 'User');
  const userEmail = String(user.email ?? '');

  return new Response(
    page(
      'Signed In',
      `<div class="text-emerald-400 mb-4">
        <svg class="w-16 h-16 mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>
        <h1 class="text-2xl font-bold mb-1">Signed In!</h1>
        <p class="text-gray-400 text-sm">You can close this window.</p>
      </div>
      ${avatar ? `<img src="${avatar}" alt="" class="w-16 h-16 rounded-full mx-auto mb-3 border-2 border-white/20">` : ''}
      <p class="text-white font-semibold text-lg">${escapeHtml(userName)}</p>
      ${userEmail ? `<p class="text-gray-400 text-sm">${escapeHtml(userEmail)}</p>` : ''}
      <div class="mt-6 text-gray-500 text-xs">Redirecting back to Santui...</div>`,
    localUrl,
  ),
);
}
