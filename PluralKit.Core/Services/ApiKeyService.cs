using Autofac;

using System.Text;

using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

using NodaTime;

namespace PluralKit.Core;

public class ApiKeyService
{
    private readonly HttpClient _client = new();
    private readonly CoreConfig _cfg;
    private readonly ILifetimeScope _provider;

    public ApiKeyService(ILifetimeScope provider, CoreConfig cfg)
    {
        _cfg = cfg;
        _provider = provider;
    }

    public async Task<string?> CreateUserApiKey(SystemId systemId, string keyName, string[] keyScopes, bool check = false)
    {
        if (_cfg.InternalApiBaseUrl == null || _cfg.InternalApiToken == null)
            throw new Exception("internal API config not set!");

        if (!Uri.TryCreate(new Uri(_cfg.InternalApiBaseUrl), "/internal/apikey/user", out var uri))
            throw new Exception("internal API base invalid!?");

        var repo = _provider.Resolve<ModelRepository>();
        var system = await repo.GetSystem(systemId);
        if (system == null)
            return null;

        var req = new JObject();
        req.Add("check", check);
        req.Add("system", system.Id.Value);
        req.Add("name", keyName);
        req.Add("scopes", new JArray(keyScopes));

        var body = new StringContent(JsonConvert.SerializeObject(req), Encoding.UTF8, "application/json");
        var res = await _client.PostAsync(uri, body);
        var data = JsonConvert.DeserializeObject<JObject>(await res.Content.ReadAsStringAsync());

        if (data.ContainsKey("error"))
            throw new Exception(data.Value<string>("error"));

        if (data.Value<bool>("valid") != true)
            throw new Exception("unknown validation error");

        if (!data.ContainsKey("token"))
            return null;

        return data.Value<string>("token");
    }
}