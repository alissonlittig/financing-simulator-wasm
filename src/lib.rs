//! Loan maths: SAC (constant amortization) vs Price (French / constant payment).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum System {
    Sac,
    Price,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            System::Sac => "SAC",
            System::Price => "Price",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub n: u32,
    pub payment: f64,
    pub interest: f64,
    pub amortization: f64,
    pub balance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub first: f64,
    pub last: f64,
    pub total_interest: f64,
    pub total_paid: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calc {
    pub principal: f64,
    pub sac: Vec<Row>,
    pub price: Vec<Row>,
    pub sac_sum: Summary,
    pub price_sum: Summary,
}

/// Annual effective rate (percent) to monthly decimal rate.
pub fn monthly_rate(annual_pct: f64) -> f64 {
    (1.0 + annual_pct / 100.0).powf(1.0 / 12.0) - 1.0
}

pub fn schedule(sys: System, principal: f64, rate: f64, n: u32) -> Vec<Row> {
    let nf = n as f64;
    let pmt = if rate == 0.0 {
        principal / nf
    } else {
        principal * rate / (1.0 - (1.0 + rate).powf(-nf))
    };
    let mut bal = principal;
    let mut rows = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let interest = bal * rate;
        let amort = if i == n {
            bal // absorbs float drift: the loan ends at exactly zero
        } else {
            match sys {
                System::Sac => principal / nf,
                System::Price => pmt - interest,
            }
        };
        bal = (bal - amort).max(0.0);
        rows.push(Row { n: i, payment: interest + amort, interest, amortization: amort, balance: bal });
    }
    rows
}

pub fn summarize(rows: &[Row]) -> Summary {
    Summary {
        first: rows.first().map_or(0.0, |r| r.payment),
        last: rows.last().map_or(0.0, |r| r.payment),
        total_interest: rows.iter().map(|r| r.interest).sum(),
        total_paid: rows.iter().map(|r| r.payment).sum(),
    }
}

/// First month in which the SAC payment is not above the Price payment.
pub fn break_even(sac: &[Row], price: &[Row]) -> Option<u32> {
    sac.iter().zip(price).find(|(s, p)| s.payment <= p.payment + 0.005).map(|(s, _)| s.n)
}

pub fn compare(price: f64, down: f64, annual_pct: f64, months: f64) -> Result<Calc, String> {
    if !(price > 0.0) {
        return Err("Informe o valor do bem.".into());
    }
    if !(down >= 0.0) || down >= price {
        return Err("A entrada deve ser menor que o valor do bem.".into());
    }
    if !(0.0..=100.0).contains(&annual_pct) {
        return Err("A taxa anual deve ficar entre 0% e 100%.".into());
    }
    if months.fract() != 0.0 || !(1.0..=600.0).contains(&months) {
        return Err("O prazo deve ser um número inteiro de 1 a 600 meses.".into());
    }
    let principal = price - down;
    let n = months as u32;
    let r = monthly_rate(annual_pct);
    let sac = schedule(System::Sac, principal, r, n);
    let pr = schedule(System::Price, principal, r, n);
    Ok(Calc { principal, sac_sum: summarize(&sac), price_sum: summarize(&pr), sac, price: pr })
}

/// "1.234,56" (pt-BR, two decimals).
pub fn num(v: f64) -> String {
    let cents = (v.abs() * 100.0).round() as u64;
    let s = (cents / 100).to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    format!("{}{},{:02}", if v < 0.0 { "-" } else { "" }, out, cents % 100)
}

pub fn brl(v: f64) -> String {
    format!("R$ {}", num(v))
}

fn short(v: f64) -> String {
    let s = if v >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if v >= 1e3 {
        format!("{:.0}k", v / 1e3)
    } else {
        format!("{:.0}", v)
    };
    s.replace('.', ",")
}

pub fn csv(calc: &Calc) -> String {
    let mut out = String::from("system,month,payment,interest,amortization,balance\n");
    for (sys, rows) in [(System::Sac, &calc.sac), (System::Price, &calc.price)] {
        for r in rows.iter() {
            out.push_str(&format!(
                "{},{},{:.2},{:.2},{:.2},{:.2}\n",
                sys.name(), r.n, r.payment, r.interest, r.amortization, r.balance
            ));
        }
    }
    out
}

pub struct Series<'a> {
    pub class: &'a str,
    pub values: Vec<f64>,
}

fn nice_ceil(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    let mag = 10f64.powf(max.log10().floor());
    let norm = max / mag;
    let step = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0]
        .into_iter()
        .find(|s| *s >= norm - 1e-9)
        .unwrap_or(10.0);
    step * mag
}

/// Line chart as an SVG string. `x_start` is the month of the first value.
pub fn chart_svg(label: &str, series: &[Series], x_start: u32) -> String {
    let (w, h, l, r, t, b) = (640.0_f64, 260.0_f64, 54.0_f64, 14.0_f64, 12.0_f64, 30.0_f64);
    let max = series.iter().flat_map(|s| s.values.iter().copied()).fold(0.0, f64::max);
    let top = nice_ceil(max);
    let len = series.first().map_or(1, |s| s.values.len()).max(1);
    let den = (len.max(2) - 1) as f64;
    let x = |i: usize| l + (w - l - r) * i as f64 / den;
    let y = |v: f64| t + (h - t - b) * (1.0 - v / top);
    let mut svg = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{label}\" xmlns=\"http://www.w3.org/2000/svg\">"
    );
    for k in 0..=4 {
        let v = top * k as f64 / 4.0;
        let yy = y(v);
        svg.push_str(&format!(
            "<line class=\"grid\" x1=\"{l}\" x2=\"{}\" y1=\"{yy:.1}\" y2=\"{yy:.1}\"/><text class=\"axis\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            w - r, l - 8.0, yy + 4.0, short(v)
        ));
    }
    for (k, anchor) in [(0usize, "start"), (len / 2, "middle"), (len - 1, "end")] {
        svg.push_str(&format!(
            "<text class=\"axis\" x=\"{:.1}\" y=\"{}\" text-anchor=\"{anchor}\">mês {}</text>",
            x(k), h - 8.0, x_start as usize + k
        ));
    }
    for s in series {
        let d: Vec<String> = s
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{}{:.1} {:.1}", if i == 0 { "M" } else { "L" }, x(i), y(*v)))
            .collect();
        svg.push_str(&format!("<path class=\"line {}\" d=\"{}\"/>", s.class, d.join(" ")));
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn price_matches_reference_payment() {
        // 100k at 1%/month over 12 months -> 8,884.88 (standard annuity table)
        let rows = schedule(System::Price, 100_000.0, 0.01, 12);
        assert!(close(rows[0].payment, 8884.88));
        assert!(close(rows[5].payment, 8884.88));
        assert!(close(summarize(&rows).total_interest, 6618.55));
    }

    #[test]
    fn sac_total_interest_formula() {
        // interest = P * r * (n + 1) / 2
        let rows = schedule(System::Sac, 100_000.0, 0.01, 12);
        assert!(close(summarize(&rows).total_interest, 6500.0));
        assert!(close(rows[0].payment, 100_000.0 / 12.0 + 1000.0));
    }

    #[test]
    fn both_end_at_zero_and_amortize_principal() {
        for sys in [System::Sac, System::Price] {
            let rows = schedule(sys, 350_000.0, monthly_rate(10.5), 360);
            assert_eq!(rows.last().unwrap().balance, 0.0);
            assert!(close(rows.iter().map(|r| r.amortization).sum::<f64>(), 350_000.0));
        }
    }

    #[test]
    fn zero_rate_is_linear() {
        let rows = schedule(System::Price, 1200.0, 0.0, 12);
        assert!(rows.iter().all(|r| close(r.payment, 100.0) && r.interest == 0.0));
    }

    #[test]
    fn sac_cheaper_overall_and_crosses_price() {
        let c = compare(500_000.0, 100_000.0, 10.5, 360.0).unwrap();
        assert!(c.sac_sum.total_interest < c.price_sum.total_interest);
        assert!(c.sac_sum.first > c.price_sum.first);
        let m = break_even(&c.sac, &c.price).unwrap();
        assert!(m > 1 && m < 360);
    }

    #[test]
    fn validation() {
        assert!(compare(0.0, 0.0, 10.0, 12.0).is_err());
        assert!(compare(100.0, 100.0, 10.0, 12.0).is_err());
        assert!(compare(100.0, 10.0, 150.0, 12.0).is_err());
        assert!(compare(100.0, 10.0, 10.0, 12.5).is_err());
        assert!(compare(100.0, 10.0, 10.0, 601.0).is_err());
        assert!(compare(100.0, 10.0, f64::NAN, 12.0).is_err());
    }

    #[test]
    fn formatting() {
        assert_eq!(brl(1234567.891), "R$ 1.234.567,89");
        assert_eq!(num(0.5), "0,50");
        assert_eq!(num(999.0), "999,00");
        assert_eq!(short(2_500_000.0), "2,5M");
    }

    #[test]
    fn csv_has_both_systems() {
        let c = compare(1000.0, 0.0, 12.0, 3.0).unwrap();
        let out = csv(&c);
        assert_eq!(out.lines().count(), 1 + 6);
        assert!(out.starts_with("system,month,payment"));
        assert!(out.contains("\nPrice,3,"));
    }

    #[test]
    fn chart_contains_paths() {
        let s = chart_svg("x", &[Series { class: "s-sac", values: vec![3.0, 2.0, 1.0] }], 1);
        assert!(s.contains("<path") && s.contains("mês 3"));
        // single-point series must not divide by zero
        assert!(!chart_svg("x", &[Series { class: "a", values: vec![5.0] }], 1).contains("NaN"));
    }
}
