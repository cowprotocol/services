//! Module emulating the pow function as found in the Balancer contract code.
// The original contract code can be found at:
// https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/solidity-utils/contracts/math/LogExpMath.sol

use super::super::error::Error;
use ethcontract::{I256, U256};
use lazy_static::lazy_static;
use std::convert::TryFrom;

/// Fixed point number stored in a type of bit size 256 that stores exactly 18
/// decimal digits.
type Ufixed256x18 = U256;

lazy_static! {
    static ref ONE_18: I256 = I256::exp10(18);
    static ref ONE_20: I256 = I256::exp10(20);
    static ref ONE_36: I256 = I256::exp10(36);
    static ref UFIXED256X18_ONE: Ufixed256x18 = U256::try_from(*ONE_18).unwrap();
    static ref MAX_NATURAL_EXPONENT: I256 = ONE_18.checked_mul(I256::from(130_i128)).unwrap();
    static ref MIN_NATURAL_EXPONENT: I256 = ONE_18.checked_mul(I256::from(-41_i128)).unwrap();
    static ref LN_36_LOWER_BOUND: I256 = ONE_18.checked_sub(I256::exp10(17)).unwrap();
    static ref LN_36_UPPER_BOUND: I256 = ONE_18.checked_add(I256::exp10(17)).unwrap();
    static ref MILD_EXPONENT_BOUND: Ufixed256x18 = (U256::one() << 254_u32)
        .checked_div(U256::try_from(*ONE_20).unwrap())
        .unwrap();
}
fn constant_x_20(i: u32) -> I256 {
    match i {
        2 => 3_200_000_000_000_000_000_000_i128,
        3 => 1_600_000_000_000_000_000_000_i128,
        4 => 800_000_000_000_000_000_000_i128,
        5 => 400_000_000_000_000_000_000_i128,
        6 => 200_000_000_000_000_000_000_i128,
        7 => 100_000_000_000_000_000_000_i128,
        8 => 50_000_000_000_000_000_000_i128,
        9 => 25_000_000_000_000_000_000_i128,
        10 => 12_500_000_000_000_000_000_i128,
        11 => 6_250_000_000_000_000_000_i128,
        _ => panic!("Constant not provided"),
    }
    .into()
}
fn constant_x_18(i: u32) -> I256 {
    match i {
        0 => 128_000_000_000_000_000_000_i128,
        1 => 64_000_000_000_000_000_000_i128,
        _ => panic!("Constant not provided"),
    }
    .into()
}
fn constant_a_20(i: u32) -> I256 {
    match i {
        2 => 7_896_296_018_268_069_516_100_000_000_000_000_i128,
        3 => 888_611_052_050_787_263_676_000_000_i128,
        4 => 298_095_798_704_172_827_474_000_i128,
        5 => 5_459_815_003_314_423_907_810_i128,
        6 => 738_905_609_893_065_022_723_i128,
        7 => 271_828_182_845_904_523_536_i128,
        8 => 164_872_127_070_012_814_685_i128,
        9 => 128_402_541_668_774_148_407_i128,
        10 => 113_314_845_306_682_631_683_i128,
        11 => 106_449_445_891_785_942_956_i128,
        _ => panic!("Constant not provided"),
    }
    .into()
}
fn constant_a_18(i: u32) -> I256 {
    match i {
        0 => {
            I256::from_dec_str("38877084059945950922200000000000000000000000000000000000").unwrap()
        }
        1 => 6_235_149_080_811_616_882_910_000_000_i128.into(),
        _ => panic!("Constant not provided"),
    }
}

pub fn pow(x: Ufixed256x18, y: Ufixed256x18) -> Result<Ufixed256x18, Error> {
    if y == U256::zero() {
        return Ok(*UFIXED256X18_ONE);
    }
    if x == U256::zero() {
        return Ok(U256::zero());
    }

    let x_int256 = match I256::try_from(x) {
        Ok(x) => x,
        Err(_) => return Err(Error::XOutOfBounds),
    };

    let y_int256 = if y < *MILD_EXPONENT_BOUND {
        I256::try_from(y).unwrap()
    } else {
        return Err(Error::YOutOfBounds);
    };

    let mut logx_times_y = if *LN_36_LOWER_BOUND < x_int256 && x_int256 < *LN_36_UPPER_BOUND {
        let ln_36_x = _ln_36(x_int256);
        (ln_36_x / *ONE_18) * y_int256 + ((ln_36_x % *ONE_18) * y_int256) / *ONE_18
    } else {
        _ln(x_int256) * y_int256
    };
    logx_times_y /= *ONE_18;

    if !(*MIN_NATURAL_EXPONENT <= logx_times_y && logx_times_y <= *MAX_NATURAL_EXPONENT) {
        return Err(Error::ProductOutOfBounds);
    }

    exp(logx_times_y).map(|v| v.into_raw())
}

fn exp(mut x: I256) -> Result<I256, Error> {
    if !(x >= *MIN_NATURAL_EXPONENT && x <= *MAX_NATURAL_EXPONENT) {
        return Err(Error::InvalidExponent);
    }

    if x < I256::zero() {
        return Ok((*ONE_18 * *ONE_18) / exp(-x)?);
    }

    let first_an;
    if x >= constant_x_18(0) {
        x -= constant_x_18(0);
        first_an = constant_a_18(0);
    } else if x >= constant_x_18(1) {
        x -= constant_x_18(1);
        first_an = constant_a_18(1);
    } else {
        first_an = 1.into();
    }

    x *= 100.into();

    let mut product = *ONE_20;
    for i in 2..=9 {
        if x >= constant_x_20(i) {
            x -= constant_x_20(i);
            product = (product * constant_a_20(i)) / *ONE_20;
        }
    }

    let mut series_sum = *ONE_20;
    let mut term = x;
    series_sum += term;

    for i in 2..=12 {
        term = ((term * x) / *ONE_20) / i.into();
        series_sum += term;
    }

    Ok((((product * series_sum) / *ONE_20) * first_an) / 100.into())
}

fn _ln(mut a: I256) -> I256 {
    if a < *ONE_18 {
        return -_ln((*ONE_18 * *ONE_18) / a);
    }

    let mut sum = I256::zero();
    for i in 0..=1 {
        if a >= constant_a_18(i) * *ONE_18 {
            a /= constant_a_18(i);
            sum += constant_x_18(i);
        }
    }

    sum *= 100.into();
    a *= 100.into();

    for i in 2..=11 {
        if a >= constant_a_20(i) {
            a = (a * *ONE_20) / constant_a_20(i);
            sum += constant_x_20(i);
        }
    }

    let z = ((a - *ONE_20) * *ONE_20) / (a + *ONE_20);
    let z_squared = (z * z) / *ONE_20;

    let mut num = z;
    let mut series_sum = num;

    for i in (3..=11).step_by(2) {
        num = (num * z_squared) / *ONE_20;
        series_sum += num / i.into();
    }

    series_sum *= 2.into();

    (sum + series_sum) / 100.into()
}

fn _ln_36(mut x: I256) -> I256 {
    x *= *ONE_18;

    let z = ((x - *ONE_36) * *ONE_36) / (x + *ONE_36);
    let z_squared = (z * z) / *ONE_36;

    let mut num = z;
    let mut series_sum = num;

    for i in (3..=15).step_by(2) {
        num = (num * z_squared) / *ONE_36;
        series_sum += num / i.into();
    }

    series_sum * 2.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    // Compare computed constants with smart contract code.
    fn logexpmath_contract_constants(
        code: &str,
        constant_x: fn(u32) -> I256,
        constant_a: fn(u32) -> I256,
    ) {
        for line in code.split('\n').filter(|line| !line.trim().is_empty()) {
            let re = Regex::new(r".* ([ax])([\d]+) = (\d+);.*$").unwrap();
            let cap = re.captures_iter(line).next().unwrap();
            match &cap[1] {
                "x" => assert_eq!(&cap[3], format!("{}", constant_x(cap[2].parse().unwrap()))),
                "a" => assert_eq!(&cap[3], format!("{}", constant_a(cap[2].parse().unwrap()))),
                _ => panic!("invalid constant"),
            }
        }
    }

    #[test]
    fn logexpmath_contract_constants_20() {
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/solidity-utils/contracts/math/LogExpMath.sol#L67-L86
        let code = "
    int256 constant x2 = 3200000000000000000000; // 2ˆ5
    int256 constant a2 = 7896296018268069516100000000000000; // eˆ(x2)
    int256 constant x3 = 1600000000000000000000; // 2ˆ4
    int256 constant a3 = 888611052050787263676000000; // eˆ(x3)
    int256 constant x4 = 800000000000000000000; // 2ˆ3
    int256 constant a4 = 298095798704172827474000; // eˆ(x4)
    int256 constant x5 = 400000000000000000000; // 2ˆ2
    int256 constant a5 = 5459815003314423907810; // eˆ(x5)
    int256 constant x6 = 200000000000000000000; // 2ˆ1
    int256 constant a6 = 738905609893065022723; // eˆ(x6)
    int256 constant x7 = 100000000000000000000; // 2ˆ0
    int256 constant a7 = 271828182845904523536; // eˆ(x7)
    int256 constant x8 = 50000000000000000000; // 2ˆ-1
    int256 constant a8 = 164872127070012814685; // eˆ(x8)
    int256 constant x9 = 25000000000000000000; // 2ˆ-2
    int256 constant a9 = 128402541668774148407; // eˆ(x9)
    int256 constant x10 = 12500000000000000000; // 2ˆ-3
    int256 constant a10 = 113314845306682631683; // eˆ(x10)
    int256 constant x11 = 6250000000000000000; // 2ˆ-4
    int256 constant a11 = 106449445891785942956; // eˆ(x11)
    ";
        logexpmath_contract_constants(code, constant_x_20, constant_a_20);
    }

    #[test]
    fn logexpmath_contract_constants_18() {
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/6c9e24e22d0c46cca6dd15861d3d33da61a60b98/pkg/solidity-utils/contracts/math/LogExpMath.sol#L61-L64
        let code = "
        int256 constant x0 = 128000000000000000000; // 2ˆ7
        int256 constant a0 = 38877084059945950922200000000000000000000000000000000000; // eˆ(x0) (no decimals)
        int256 constant x1 = 64000000000000000000; // 2ˆ6
        int256 constant a1 = 6235149080811616882910000000; // eˆ(x1) (no decimals)
    ";
        logexpmath_contract_constants(code, constant_x_18, constant_a_18);
    }

    // The expected output for the tested functions was generated by running the
    // following instructions after cloning and installing the repo at
    // github.com/balancer-labs/balancer-v2-monorepo, commit
    // 6c9e24e22d0c46cca6dd15861d3d33da61a60b98:
    // ```
    // $ cd pkg/solidity-utils/
    // $ cp contracts/math/LogExpMath.sol contracts/math/LogExpMathTest.sol
    // $ sed --in-place -E 's/library LogExpMath/contract LogExpMathTest/' contracts/math/LogExpMathTest.sol
    // $ sed --in-place -E 's/(private|internal)/public/' contracts/math/LogExpMathTest.sol
    // $ yarn hardhat console
    // > const logexpmath = await (await ethers.getContractFactory("LogExpMathTest")).deploy()
    // > const generateOk = async (fn, input) => JSON.stringify(await Promise.all(input.map(async (i) => (await logexpmath[fn](...[i].flat())).toString())))
    // > const generateErr = async (fn, input) => JSON.stringify(await Promise.all(input.map(async (i) => (await logexpmath[fn](...[i].flat()).then(() => {throw new Error("input does not throw")}, e => e.message.slice(-3))))))
    // ```
    // Every test has an input vector that can be directly copied into the
    // Hardhat console and custom instructions to print the output vector used
    // in the test code.

    #[test]
    fn _ln_success() {
        let input = [
            "1",
            "100",
            "1000000",
            "100000000000",
            "1000000000000000000", // 10**18
            "100000000000000000000000",
            "100000000000000000000000000000",
            "100000000000000000000000000000000000",
            "100000000000000000000000000000000000000000",
            "100000000000000000000000000000000000000000000000000000000000000000",
            "10000000000000000000000000000000000000000000000000000000000000000000000000000", // ≈2**255
        ];
        // generated with `await generateOk("_ln", input)`
        let output = [
            "-41446531673892822312",
            "-36841361487904730944",
            "-27631021115928548208",
            "-16118095650958319788",
            "0",
            "11512925464970228420",
            "25328436022934502524",
            "39143946580898776628",
            "52959457138863050732",
            "108221499370720147148",
            "133549935393654649672",
        ];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(
                _ln(I256::from_dec_str(i).unwrap()),
                I256::from_dec_str(o).unwrap()
            );
        }
    }

    #[test]
    fn _ln_36_success() {
        let input = [
            "900000000000000000", // LN_36_LOWER_BOUND
            "950000000000000000",
            "999999999999999999",
            "1000000000000000000", // 10**18
            "1000000000000000001",
            "1050000000000000000",
            "1100000000000000000", // LN_36_UPPER_BOUND
        ];
        // generated with `await generateOk("_ln_36", input)`
        let output = [
            "-105360515657826301227479460574005190",
            "-51293294387550533426196144149312054",
            "-1000000000000000000",
            "0",
            "999999999999999998",
            "48790164169432003065374404178136230",
            "95310179804324860043948199225536944",
        ];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(
                _ln_36(I256::from_dec_str(i).unwrap()),
                I256::from_dec_str(o).unwrap()
            );
        }
    }

    #[test]
    fn exp_success() {
        let input = [
            "-41000000000000000000", // MIN_NATURAL_EXPONENT
            "-10000000000000000000",
            "-1000000000000000000", // -1
            "-100000000",
            "-1",
            "0",
            "1",
            "100000000",
            "999999999999999999",
            "1000000000000000000", // 1
            "1000000000000000001",
            "10000000000000000000",
            "100000000000000000000",
            "130000000000000000000", // MAX_NATURAL_EXPONENT
        ];
        // generated with `await generateOk("exp", input)`
        let output = [
            "1",
            "45399929762484",
            "367879441171442321",
            "999999999900000000",
            "999999999999999999",
            "1000000000000000000",
            "1000000000000000001",
            "1000000000100000000",
            "2718281828459045227",
            "2718281828459045235",
            "2718281828459045238",
            "22026465794806716516930",
            "26881171418161354484131967259153438289195652545281114830700000",
            "287264955081783193326519143742863858051506000000000000000000000000000000000",
        ];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(
                exp(I256::from_dec_str(i).unwrap()).unwrap(),
                I256::from_dec_str(o).unwrap()
            );
        }
    }

    #[test]
    fn exp_error() {
        let input = [
            "-41000000000000000001",  // MIN_NATURAL_EXPONENT - 1
            "130000000000000000001",  // MAX_NATURAL_EXPONENT + 1
            "-130000000000000000001", // -(MAX_NATURAL_EXPONENT + 1)
        ];
        // generated with `await generateErr("exp", input)`
        let output = ["009", "009", "009"];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(exp(I256::from_dec_str(i).unwrap()).unwrap_err(), o.into());
        }
    }

    #[test]
    fn pow_error() {
        let input = [
            [
                "57896044618658097711785492504343953926634992332820282019728792003956564819968", // I256::MAX + 1
                "1",
            ],
            [
                "1000000000000000000",
                "289480223093290488558927462521719769633174961664101410098", // MILD_EXPONENT_BOUND
            ],
            [
                "287300000000000000000000000000000000000000000000000000000000000000000000000", // slightly larger than f64::exp(MAX_NATURAL_EXPONENT)
                "1000000000000000000",                                                         // 1
            ],
            [
                "1250152866",          // slightly smaller than f64::exp(-MIN_NATURAL_EXPONENT/2)
                "2000000000000000000", // 2
            ],
            [
                "115792089237316195423570985008687907853269984665640564039457584007913129639935", // U256::MAX
                "1",
            ],
            [
                "1",
                "115792089237316195423570985008687907853269984665640564039457584007913129639935", // U256::MAX
            ],
            [
                "130000000000000000001", // MAX_NATURAL_EXPONENT + 1
                "130000000000000000001",
            ],
            ["1", "130000000000000000001"],
        ];
        // generated with `await generateErr("pow", input)`
        let output = ["006", "007", "008", "008", "006", "007", "008", "008"];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(
                pow(
                    U256::from_dec_str(i[0]).unwrap(),
                    U256::from_dec_str(i[1]).unwrap()
                )
                .unwrap_err(),
                o.into()
            );
        }
    }
    #[test]
    fn pow_success() {
        let input = [
            [
                "57896044618658097711785492504343953926634992332820282019728792003956564819967", // I256::MAX
                "1",
            ],
            [
                "1000000000000000000",
                "289480223093290488558927462521719769633174961664101410097", // MILD_EXPONENT_BOUND - 1
            ],
            [
                "287200000000000000000000000000000000000000000000000000000000000000000000000", // slightly smaller than f64::exp(MAX_NATURAL_EXPONENT)
                "1000000000000000000",                                                         // 1
            ],
            [
                "1250152867",          // slightly larger than f64::exp(-MIN_NATURAL_EXPONENT/2)
                "2000000000000000000", // 2
            ],
            ["0", "1000000000000000000000"],
            ["0", "0"],
            ["1000000000000000000", "0"],
            ["2000000000000000000", "2000000000000000000"],
        ];
        // generated with `await generateOk("pow", input)`
        let output = [
            "1000000000000000135",
            "1000000000000000000",
            "287199999999999999375313920267432160096964000000000000000000000000000000000",
            "1",
            "0",
            "1000000000000000000",
            "1000000000000000000",
            "3999999999999999996",
        ];

        assert_eq!(input.len(), output.len());
        for (i, &o) in input.iter().zip(output.iter()) {
            assert_eq!(
                pow(
                    U256::from_dec_str(i[0]).unwrap(),
                    U256::from_dec_str(i[1]).unwrap()
                )
                .unwrap(),
                U256::from_dec_str(o).unwrap()
            );
        }
    }

    #[test]
    #[should_panic]
    fn constant_x_20_panic() {
        constant_x_20(12);
    }

    #[test]
    #[should_panic]
    fn constant_x_18_panic() {
        constant_x_18(2);
    }

    #[test]
    #[should_panic]
    fn constant_a_20_panic() {
        constant_a_20(12);
    }

    #[test]
    #[should_panic]
    fn constant_a_18_panic() {
        constant_a_18(2);
    }

    #[test]
    fn pow_alternate_routes() {
        assert_eq!(
            pow(
                U256::from_dec_str("0").unwrap(),
                U256::from_dec_str("0").unwrap()
            ),
            Ok(*UFIXED256X18_ONE)
        );
        assert_eq!(
            pow(
                U256::from_dec_str("0").unwrap(),
                U256::from_dec_str("1").unwrap()
            ),
            Ok(U256::zero())
        );
        assert_eq!(pow(U256::exp10(18), U256::one()), Ok(*UFIXED256X18_ONE));
    }
}
