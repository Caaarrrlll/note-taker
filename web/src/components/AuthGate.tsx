import { useCallback, useEffect, useState, type ReactNode } from "react";

import { authApi, errorMessage, SIGNED_OUT_EVENT, type AuthStatus, type User } from "../api";
import { flushPendingSaves } from "../pendingSaves";

type Mode = "login" | "register" | "setup";

interface AuthGateProps {
  /** Rendered once signed in. */
  children: (user: User, signOut: () => void) => ReactNode;
}

/** Web only: first-run setup, sign-in and registration in front of the app. */
export function AuthGate({ children }: AuthGateProps) {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    authApi.status().then(setStatus, (e) => setLoadError(errorMessage(e)));
  }, []);

  useEffect(refresh, [refresh]);

  // The server dropped our session (expired, or signed out elsewhere).
  useEffect(() => {
    const onSignedOut = () => setStatus((s) => (s ? { ...s, user: null } : s));
    window.addEventListener(SIGNED_OUT_EVENT, onSignedOut);
    return () => window.removeEventListener(SIGNED_OUT_EVENT, onSignedOut);
  }, []);

  const signOut = useCallback(() => {
    // Save the last edits while the session still works, then sign out.
    flushPendingSaves()
      .then(() => authApi.logout())
      .finally(refresh);
  }, [refresh]);

  if (loadError) {
    return (
      <div className="empty-state" role="alert">
        <p>Can't reach the server: {loadError}</p>
        <button className="primary-button" onClick={() => location.reload()}>
          Try again
        </button>
      </div>
    );
  }
  if (!status) return null;
  if (status.user) return <>{children(status.user, signOut)}</>;
  return (
    <SignInForm
      initialMode={status.needsSetup ? "setup" : "login"}
      registrationOpen={status.registrationOpen}
      onSignedIn={(user) => setStatus({ ...status, needsSetup: false, user })}
    />
  );
}

interface SignInFormProps {
  initialMode: Mode;
  registrationOpen: boolean;
  onSignedIn: (user: User) => void;
}

function SignInForm({ initialMode, registrationOpen, onSignedIn }: SignInFormProps) {
  const [mode, setMode] = useState<Mode>(initialMode);
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const newAccount = mode !== "login";
  const title = mode === "setup" ? "Set up Note Taker" : mode === "register" ? "Create an account" : "Sign in";

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const user =
        mode === "setup"
          ? await authApi.setup(email, displayName, password)
          : mode === "register"
            ? await authApi.register(email, displayName, password)
            : await authApi.login(email, password);
      onSignedIn(user);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auth-page">
      <form
        className="auth-card"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <h1>{title}</h1>
        {mode === "setup" && (
          <p className="hint">This server has no accounts yet. Create the first one; registration stays closed after that.</p>
        )}
        <label className="field">
          <span>Email</span>
          <input type="email" autoComplete="email" required autoFocus value={email} onChange={(e) => setEmail(e.target.value)} />
        </label>
        {newAccount && (
          <label className="field">
            <span>Name</span>
            <input autoComplete="name" required value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
          </label>
        )}
        <label className="field">
          <span>Password</span>
          <input
            type="password"
            autoComplete={newAccount ? "new-password" : "current-password"}
            required
            minLength={newAccount ? 8 : undefined}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <button type="submit" className="primary-button" disabled={busy}>
          {mode === "login" ? "Sign in" : "Create account"}
        </button>
        {mode === "login" && registrationOpen && (
          <button type="button" className="text-button" onClick={() => setMode("register")}>
            No account? Create one
          </button>
        )}
        {mode === "register" && (
          <button type="button" className="text-button" onClick={() => setMode("login")}>
            Already have an account? Sign in
          </button>
        )}
      </form>
    </div>
  );
}
