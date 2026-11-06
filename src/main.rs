use financing::*;
use leptos::*;
use wasm_bindgen::JsCast;

/// Offline CSV download through a data: URL (no Blob API needed).
fn download(name: &str, content: &str) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let href = format!("data:text/csv;charset=utf-8,{}", String::from(js_sys::encode_uri_component(content)));
    if let Ok(el) = doc.create_element("a") {
        if let Ok(a) = el.dyn_into::<web_sys::HtmlAnchorElement>() {
            a.set_href(&href);
            a.set_download(name);
            a.click();
        }
    }
}

#[component]
fn Field(
    id: &'static str,
    label: &'static str,
    hint: &'static str,
    step: &'static str,
    value: ReadSignal<f64>,
    set: WriteSignal<f64>,
) -> impl IntoView {
    view! {
        <div class="field">
            <label for=id>{label}</label>
            <input
                id=id
                type="number"
                inputmode="decimal"
                min="0"
                step=step
                prop:value=move || value.get().to_string()
                on:change=move |ev| {
                    if let Ok(v) = event_target_value(&ev).trim().parse::<f64>() {
                        set.set(v);
                    }
                }
            />
            <small>{hint}</small>
        </div>
    }
}

#[component]
fn SystemCard(name: &'static str, class: &'static str, s: Summary) -> impl IntoView {
    view! {
        <article class=format!("card sys {class}")>
            <h3><i></i>{name}</h3>
            <dl>
                <div><dt>"Primeira parcela"</dt><dd>{brl(s.first)}</dd></div>
                <div><dt>"Última parcela"</dt><dd>{brl(s.last)}</dd></div>
                <div><dt>"Juros totais"</dt><dd>{brl(s.total_interest)}</dd></div>
                <div><dt>"Total pago"</dt><dd>{brl(s.total_paid)}</dd></div>
            </dl>
        </article>
    }
}

fn results(c: Calc, sys: ReadSignal<System>, set_sys: WriteSignal<System>) -> View {
    let saved = c.price_sum.total_interest - c.sac_sum.total_interest;
    let pct = if c.price_sum.total_interest > 0.0 { saved / c.price_sum.total_interest * 100.0 } else { 0.0 };
    let start_gap = c.sac_sum.first - c.price_sum.first;
    let cross = break_even(&c.sac, &c.price).filter(|m| *m > 1);

    let pay_chart = chart_svg(
        "Valor da parcela mês a mês: SAC cai, Price é constante",
        &[
            Series { class: "s-sac", values: c.sac.iter().map(|r| r.payment).collect() },
            Series { class: "s-price", values: c.price.iter().map(|r| r.payment).collect() },
        ],
        1,
    );
    let bal = |rows: &[Row]| {
        let mut v = vec![c.principal];
        v.extend(rows.iter().map(|r| r.balance));
        v
    };
    let bal_chart = chart_svg(
        "Saldo devedor ao longo do prazo",
        &[Series { class: "s-sac", values: bal(&c.sac) }, Series { class: "s-price", values: bal(&c.price) }],
        0,
    );

    let csv_text = csv(&c);
    let table_calc = c.clone();

    view! {
        <section class="verdict card" aria-live="polite">
            <p class="eyebrow">"Resumo da comparação"</p>
            <p class="big">
                "No SAC você paga "<strong>{brl(saved)}</strong>" a menos de juros"
                <span class="pill">{format!("{:.1}%", pct)}</span>
            </p>
            <p class="muted">
                {format!("Em troca, a primeira parcela do SAC é {} maior que a da Price.", brl(start_gap))}
                {match cross {
                    Some(m) => format!(" A parcela do SAC fica abaixo da Price a partir do mês {m}."),
                    None => String::new(),
                }}
            </p>
        </section>

        <section class="cols" aria-label="Comparação">
            <SystemCard name="SAC" class="sac" s=c.sac_sum.clone() />
            <SystemCard name="Price" class="price" s=c.price_sum.clone() />
        </section>

        <section class="cols" aria-label="Gráficos">
            <figure class="card">
                <figcaption>"Parcela mês a mês"</figcaption>
                <div inner_html=pay_chart></div>
            </figure>
            <figure class="card">
                <figcaption>"Saldo devedor"</figcaption>
                <div inner_html=bal_chart></div>
            </figure>
        </section>
        <p class="legend" aria-hidden="true">
            <span class="key sac"><i></i>"SAC"</span>
            <span class="key price"><i></i>"Price"</span>
        </p>

        <section class="card">
            <div class="table-head">
                <h2>"Tabela de parcelas"</h2>
                <div class="seg" role="group" aria-label="Sistema da tabela">
                    <button
                        type="button"
                        aria-pressed=move || (sys.get() == System::Sac).to_string()
                        on:click=move |_| set_sys.set(System::Sac)
                    >"SAC"</button>
                    <button
                        type="button"
                        aria-pressed=move || (sys.get() == System::Price).to_string()
                        on:click=move |_| set_sys.set(System::Price)
                    >"Price"</button>
                </div>
                <button type="button" class="btn" on:click=move |_| download("financiamento.csv", &csv_text)>
                    "Exportar CSV"
                </button>
            </div>
            <p class="muted">"Valores em reais. Role a tabela para ver todos os meses."</p>
            <div class="table-wrap" tabindex="0" role="region" aria-label="Tabela de parcelas">
                <table>
                    <thead>
                        <tr>
                            <th>"Mês"</th>
                            <th>"Parcela"</th>
                            <th>"Juros"</th>
                            <th>"Amort."</th>
                            <th>"Saldo"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            let rows = if sys.get() == System::Sac { &table_calc.sac } else { &table_calc.price };
                            rows.iter()
                                .map(|r| {
                                    view! {
                                        <tr>
                                            <td>{r.n}</td>
                                            <td>{num(r.payment)}</td>
                                            <td>{num(r.interest)}</td>
                                            <td>{num(r.amortization)}</td>
                                            <td>{num(r.balance)}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()
                        }}
                    </tbody>
                </table>
            </div>
        </section>
    }
    .into_view()
}

#[component]
fn App() -> impl IntoView {
    let (price, set_price) = create_signal(500_000.0_f64);
    let (down, set_down) = create_signal(100_000.0_f64);
    let (rate, set_rate) = create_signal(10.5_f64);
    let (months, set_months) = create_signal(360.0_f64);
    let (sys, set_sys) = create_signal(System::Sac);

    let calc = create_memo(move |_| compare(price.get(), down.get(), rate.get(), months.get()));

    view! {
        <header class="top">
            <div class="brand">
                <span class="logo">"F"</span>
                "Financiamento"
            </div>
            <span class="tag">"Rust + WebAssembly"</span>
        </header>
        <main>
            <div class="page-head">
                <p class="eyebrow">"SAC x Price"</p>
                <h1>"Quanto custa cada sistema de amortização?"</h1>
                <p class="muted lead">
                    "Altere os valores e veja parcelas, juros e saldo devedor dos dois sistemas lado a lado. Todo o cálculo roda no seu navegador."
                </p>
            </div>

            <form class="card form" on:submit=|ev| ev.prevent_default()>
                <Field id="f-price" label="Valor do bem (R$)" hint="Preço total do imóvel ou veículo" step="1000" value=price set=set_price />
                <Field id="f-down" label="Entrada (R$)" hint="Quanto você paga à vista" step="1000" value=down set=set_down />
                <Field id="f-rate" label="Taxa anual (%)" hint="Efetiva, ao ano" step="0.1" value=rate set=set_rate />
                <Field id="f-months" label="Prazo (meses)" hint="De 1 a 600" step="1" value=months set=set_months />
                <div class="chips" role="group" aria-label="Prazos comuns">
                    {[60.0_f64, 120.0, 240.0, 360.0, 420.0]
                        .into_iter()
                        .map(|m| {
                            view! {
                                <button
                                    type="button"
                                    aria-pressed=move || (months.get() == m).to_string()
                                    on:click=move |_| set_months.set(m)
                                >{format!("{} anos", m / 12.0)}</button>
                            }
                        })
                        .collect_view()}
                </div>
            </form>

            {move || match calc.get() {
                Err(msg) => view! { <p class="alert" role="alert">{msg}</p> }.into_view(),
                Ok(c) => results(c, sys, set_sys),
            }}
        </main>
        <footer class="foot">
            "Simulação ilustrativa, sem seguros, tarifas ou correção monetária. "
            <a href="https://github.com/alissonlittig/financing-simulator-wasm">"Código no GitHub"</a>
        </footer>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}
