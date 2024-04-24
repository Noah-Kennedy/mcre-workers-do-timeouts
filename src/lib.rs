use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use worker::wasm_bindgen::JsValue;
use worker::*;

#[durable_object]
pub struct OverworkedObject {
    state: State,
    _env: Env,
}

impl OverworkedObject {
    async fn init(&mut self) -> Result<()> {
        let chunks_map = js_sys::Object::default();

        for i in 0..8 {
            js_sys::Reflect::set(
                &chunks_map,
                &JsValue::from_str(&i.to_string()),
                &serde_wasm_bindgen::to_value(&vec![0u8; 4 * 1024])?,
            )?;
        }

        self.state.storage().put_multiple_raw(chunks_map).await?;

        Ok(())
    }

    async fn dump(&mut self) -> Result<Response> {
        let mut out: Vec<Vec<u8>> = Vec::with_capacity(8);

        for i in 0..8 {
            out.push(self.state.storage().get(&i.to_string()).await?);
        }

        Response::ok(format!("{out:?}"))
    }
}

#[durable_object]
impl DurableObject for OverworkedObject {
    fn new(state: State, env: Env) -> Self {
        Self { state, _env: env }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match req.path().as_str() {
            "/init" => {
                self.init().await?;
                self.dump().await
            }
            "/dump" => self.dump().await,
            _ => Response::error("Invalid path", 404),
        }
    }
}

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    let namespace = env.durable_object("OVERWORKED")?;
    let stub = namespace.id_from_name("A")?.get_stub()?;

    if let Ok(n) = req.path()[1..].parse::<usize>() {
        let mut fu = FuturesUnordered::new();

        for _ in 0..n {
            fu.push(stub.fetch_with_str(
                "https://mcre-workers-do-timeouts.noah-s-cf-ent-account.workers.dev/init",
            ))
        }

        while let Some(n) = fu.next().await {
            let _ = n?;
        }

        return Response::ok(format!("Churned {n}"));
    }

    stub.fetch_with_request(req).await
}
