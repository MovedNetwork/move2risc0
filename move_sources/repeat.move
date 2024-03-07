module repeat::fib {
    fun fib(n: u32): u32 {
        let a: u32 = 0;
        let b: u32 = 1;

        if (n == 0) {
            return a;
        } else if (n == 1) {
            return b;
        };

        loop {
            let c = a + b;
            a = b;
            b = c;
            if (n == 1) break;
            n = n - 1;
        };

        b
    }

    public entry fun main(): u32 {
        fib(8)
    }
}
