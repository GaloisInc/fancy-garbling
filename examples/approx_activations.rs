use fancy_garbling::*;
use fancy_garbling::dummy::Dummy;
use fancy_garbling::util::{RngExt, modulus_with_nprimes, crt_factor, crt_inv_factor};
use itertools::Itertools;
use rand::Rng;

fn approx_relu<F,W>(b: &F, q: u128)
  where F: Fancy<Item=W>,
        W: HasModulus + Clone,
{
    let x = b.garbler_input_bundle_crt(None, q);
    let exact = b.relu(None, &x, "100%");
    let approx_999 = b.relu(None, &x, "99.9%");
    let approx_99  = b.relu(None, &x, "99%");
    b.output_bundles(None, &[exact, approx_999, approx_99]);
}

fn main() {
    let n = 100000;
    let mut rng = rand::thread_rng();

    let mut approx_999_errors = 0;
    let mut approx_99_errors  = 0;

    for _ in 0 .. n {
        let nprimes = rng.gen_range(5,9);
        let q = modulus_with_nprimes(nprimes);
        let x = rng.gen_u128() % q;
        let d = Dummy::new(&crt_factor(x,q), &[]);
        approx_relu(&d,q);
        let outs = d.get_output()
            .chunks(nprimes)
            .map(|xs| crt_inv_factor(xs,q))
            .collect_vec();

        if outs[1] != outs[0] {
            approx_999_errors += 1;
        }

        if outs[2] != outs[0] {
            approx_99_errors += 1;
        }
    }

    println!("relu 99.9% errors: {}/{} ({:.2}%)", approx_999_errors, n, 100.0 * (1.0 - (approx_999_errors as f64 / n as f64)));
    println!("relu 99% errors: {}/{} ({:.2}%)",   approx_99_errors,  n, 100.0 * (1.0 - (approx_99_errors  as f64 / n as f64)));
}