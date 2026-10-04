<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Login — OauthRS</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: sans-serif;
            background: #0f1117;
            color: #e0e0e0;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
        }
        .card {
            background: #1a1a2e;
            border: 1px solid #2a2a3e;
            border-radius: 12px;
            padding: 2.5rem;
            width: 100%;
            max-width: 400px;
        }
        h1 { font-size: 1.75rem; margin-bottom: 0.25rem; color: #fff; }
        .sub { color: #666; font-size: 0.9rem; margin-bottom: 2rem; }
        label { display: block; font-size: 0.85rem; color: #aaa; margin-bottom: 0.4rem; }
        input {
            width: 100%;
            padding: 0.75rem 1rem;
            background: #0f1117;
            border: 1px solid #2a2a3e;
            border-radius: 8px;
            color: #e0e0e0;
            font-size: 1rem;
            margin-bottom: 1.25rem;
            outline: none;
        }
        input:focus { border-color: #4a90e2; }
        button {
            width: 100%;
            padding: 0.85rem;
            background: #4a90e2;
            color: #fff;
            border: none;
            border-radius: 8px;
            font-size: 1rem;
            font-weight: bold;
            cursor: pointer;
            transition: opacity 0.2s;
        }
        button:hover { opacity: 0.85; }
        .error {
            background: #2a1a1a;
            border: 1px solid #5a2a2a;
            color: #e07070;
            padding: 0.75rem 1rem;
            border-radius: 8px;
            margin-bottom: 1.25rem;
            font-size: 0.9rem;
        }
        .footer { text-align: center; margin-top: 1.5rem; color: #555; font-size: 0.85rem; }
        .footer a { color: #4a90e2; text-decoration: none; }
    </style>
</head>
<body>
    <div class="card">
        <h1>Welcome back</h1>
        <p class="sub">Sign in to your account</p>

        {% if error %}
        <div class="error">{{ error }}</div>
        {% endif %}

        <form method="POST" action="/login">
            <label for="login">Email or Username</label>
            <input type="text" id="login" name="login" placeholder="you@example.com or username" required>

            <label for="password">Password</label>
            <input type="password" id="password" name="password" placeholder="••••••••" required>

            <button type="submit">Login</button>
        </form>

        {% if google_enabled %}
        <div style="margin-top:1.25rem;text-align:center;color:#555;font-size:0.85rem">or</div>
        <a href="/auth/google" style="display:flex;align-items:center;justify-content:center;gap:.6rem;margin-top:1rem;padding:.75rem;background:#fff;color:#333;border-radius:8px;font-weight:bold;text-decoration:none;font-size:.95rem">
            <svg width="18" height="18" viewBox="0 0 48 48"><path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/><path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/><path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/><path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.18 1.48-4.97 2.31-8.16 2.31-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/></svg>
            Continue with Google
        </a>
        {% endif %}
        <p class="footer" style="margin-top:1.25rem"><a href="/">← Back</a> &nbsp;·&nbsp; <a href="/signup">Create account</a></p>
    </div>
</body>
</html>
