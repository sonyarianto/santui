const CLIENT_ID = process.env.GOOGLE_CLIENT_ID ?? '';
const CLIENT_SECRET = process.env.GOOGLE_CLIENT_SECRET ?? '';
const BASE = 'https://santuiapp.vercel.app';

function html(body: string, redirect: string): string {
  return `<!DOCTYPE html>
<html><body>
${body}
<script>window.location.href=${JSON.stringify(redirect)}</script>
</body></html>`;
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
      html(
        '<h1>Google OAuth not configured</h1><p>Set GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET on Vercel.</p>',
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
      html(
        '<h1>Token exchange failed</h1>',
        `http://127.0.0.1:${port}/callback?error=token_exchange_failed`,
      ),
      { status: 200, headers: { 'Content-Type': 'text/html' } },
    );
  }

  const accessToken = tokens.access_token;
  if (!accessToken || typeof accessToken !== 'string') {
    return new Response(
      html(
        '<h1>No access token received</h1>',
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

  return new Response(
    html('<h1>Signed in! You can close this window.</h1>', localUrl),
    { status: 200, headers: { 'Content-Type': 'text/html' } },
  );
}
