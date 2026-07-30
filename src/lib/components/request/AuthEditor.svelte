<script lang="ts">
  import type { AuthType, EnvironmentVariable, RequestAuth } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    auth,
    variables = [],
    emptyMessage = "This request will be sent without authentication.",
    activeEnvironmentName = "",
    handleFetchOAuth2Token = undefined,
    onAuthChange = () => {}
  }: {
    auth: RequestAuth;
    variables?: EnvironmentVariable[];
    emptyMessage?: string;
    activeEnvironmentName?: string;
    handleFetchOAuth2Token?: (options: { persistToEnvironment: boolean }) => Promise<{
      accessToken: string;
      persistedToEnvironment: boolean;
      expiresIn: number | null;
      tokenType: string;
    }>;
    onAuthChange?: (auth: RequestAuth) => void;
  } = $props();

  let isFetchingOAuth2Token = $state(false);
  let oauth2FetchErrorText = $state("");
  let oauth2FetchStatusText = $state("");
  let shouldPersistOAuth2Token = $state(true);

  let canPersistOAuth2Token = $derived(Boolean(activeEnvironmentName && handleFetchOAuth2Token));

  function patchAuth(patch: Partial<RequestAuth>) {
    onAuthChange({ ...auth, ...patch });
  }

  async function fetchOAuth2Token() {
    if (!handleFetchOAuth2Token || isFetchingOAuth2Token) {
      return;
    }

    isFetchingOAuth2Token = true;
    oauth2FetchErrorText = "";
    oauth2FetchStatusText = "";

    try {
      const result = await handleFetchOAuth2Token({
        persistToEnvironment: canPersistOAuth2Token && shouldPersistOAuth2Token
      });
      patchAuth({
        type: "oauth2",
        oauth2AccessToken: result.persistedToEnvironment ? "{{oauth_access_token}}" : result.accessToken
      });
      const expiryText = result.expiresIn ? ` Expires in ${result.expiresIn}s.` : "";
      oauth2FetchStatusText = result.persistedToEnvironment
        ? `Token saved to ${activeEnvironmentName} as {{oauth_access_token}}.${expiryText}`
        : `Token fetched into this request field.${expiryText}`;
    } catch (error) {
      oauth2FetchErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isFetchingOAuth2Token = false;
    }
  }
</script>

<div class="editor-block auth-editor">
  <div class="editor-header">
    <h2>Auth</h2>
    <label class="body-mode-control">
      <span class="sr-only">Auth type</span>
      <select class="body-mode-select" value={auth.type} onchange={(event) => patchAuth({ type: event.currentTarget.value as AuthType })}>
        <option value="none">None</option>
        <option value="basic">Basic</option>
        <option value="bearer">Bearer</option>
        <option value="api-key">API key</option>
        <option value="oauth2">OAuth2</option>
      </select>
    </label>
  </div>

  {#if auth.type === "none"}
    <div class="empty-state body-empty-state">
      {emptyMessage}
    </div>
  {:else if auth.type === "basic"}
    <div class="auth-grid">
      <label>
        <span class="field-label">Username</span>
        <VariableField className="text-input" value={auth.basicUsername} {variables} onValueInput={(value) => patchAuth({ basicUsername: value })} />
      </label>
      <label>
        <span class="field-label">Password</span>
        <VariableField className="text-input" type="password" value={auth.basicPassword} {variables} onValueInput={(value) => patchAuth({ basicPassword: value })} />
      </label>
    </div>
  {:else if auth.type === "bearer"}
    <div class="auth-grid">
      <label>
        <span class="field-label">Token</span>
        <VariableField className="text-input" type="password" value={auth.bearerToken} {variables} placeholder={"{{api_token}}"} onValueInput={(value) => patchAuth({ bearerToken: value })} />
      </label>
    </div>
  {:else if auth.type === "api-key"}
    <div class="auth-grid">
      <label>
        <span class="field-label">Key</span>
        <VariableField className="text-input" value={auth.apiKeyName} {variables} onValueInput={(value) => patchAuth({ apiKeyName: value })} />
      </label>
      <label>
        <span class="field-label">Value</span>
        <VariableField className="text-input" type="password" value={auth.apiKeyValue} {variables} onValueInput={(value) => patchAuth({ apiKeyValue: value })} />
      </label>
      <label>
        <span class="field-label">Send in</span>
        <select class="text-input" value={auth.apiKeyIn} onchange={(event) => patchAuth({ apiKeyIn: event.currentTarget.value as RequestAuth["apiKeyIn"] })}>
          <option value="header">Header</option>
          <option value="query">Query parameter</option>
        </select>
      </label>
    </div>
  {:else if auth.type === "oauth2"}
    <div class="auth-grid">
      <label>
        <span class="field-label">Access token</span>
        <VariableField className="text-input" type="password" value={auth.oauth2AccessToken} {variables} placeholder={"{{oauth_access_token}}"} onValueInput={(value) => patchAuth({ oauth2AccessToken: value })} />
      </label>

      {#if handleFetchOAuth2Token}
        <label>
          <span class="field-label">Token URL</span>
          <VariableField className="text-input" value={auth.oauth2TokenUrl} {variables} placeholder={"{{oauth_token_url}}"} onValueInput={(value) => patchAuth({ oauth2TokenUrl: value })} />
        </label>
        <label>
          <span class="field-label">Client ID</span>
          <VariableField className="text-input" value={auth.oauth2ClientId} {variables} placeholder={"{{oauth_client_id}}"} onValueInput={(value) => patchAuth({ oauth2ClientId: value })} />
        </label>
        <label>
          <span class="field-label">Client secret</span>
          <VariableField className="text-input" type="password" value={auth.oauth2ClientSecret} {variables} placeholder={"{{oauth_client_secret}}"} onValueInput={(value) => patchAuth({ oauth2ClientSecret: value })} />
        </label>
        <label>
          <span class="field-label">Scope</span>
          <VariableField className="text-input" value={auth.oauth2Scope} {variables} placeholder={"{{oauth_scope}}"} onValueInput={(value) => patchAuth({ oauth2Scope: value })} />
        </label>
        <div class="auth-action-row">
          <div class="oauth2-actions">
            <button class="button-primary" type="button" onclick={fetchOAuth2Token} disabled={isFetchingOAuth2Token}>
              {isFetchingOAuth2Token ? "Fetching..." : "Fetch token"}
            </button>
            <label class={["inline-checkbox", !canPersistOAuth2Token && "inline-checkbox-disabled"]}>
              <input
                class="row-toggle"
                type="checkbox"
                checked={shouldPersistOAuth2Token && canPersistOAuth2Token}
                disabled={!canPersistOAuth2Token || isFetchingOAuth2Token}
                onchange={(event) => (shouldPersistOAuth2Token = event.currentTarget.checked)}
              />
              <span>
                {canPersistOAuth2Token
                  ? `Save to ${activeEnvironmentName} as {{oauth_access_token}}`
                  : "Activate an environment to save the token as {{oauth_access_token}}"}
              </span>
            </label>
          </div>
          {#if oauth2FetchStatusText}
            <p class="auth-status-text">{oauth2FetchStatusText}</p>
          {/if}
          {#if oauth2FetchErrorText}
            <p class="auth-error-text">{oauth2FetchErrorText}</p>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
