use alloy::sol;

sol! {
    #[derive(Debug)]
    contract UniswapV2Router02 {
        function swapExactTokensForTokens(
            uint amountIn,
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override ensure(deadline) returns (uint[] memory amounts);

        function swapTokensForExactTokens(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override ensure(deadline) returns (uint[] memory amounts);

        function swapExactETHForTokens(
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override payable ensure(deadline) returns (uint[] memory amounts);

        function swapTokensForExactETH(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override ensure(deadline) returns (uint[] memory amounts);

        function swapExactTokensForETH(
            uint amountIn,
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override ensure(deadline) returns (uint[] memory amounts);

        function swapETHForExactTokens(
            uint amountOut,
            address[] calldata path,
            address to,
            uint deadline
        ) external virtual override payable ensure(deadline) returns (uint[] memory amounts);
    }

    contract SwapRouter {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(
            ExactInputSingleParams calldata params
        ) external payable override checkDeadline(params.deadline) returns (uint256 amountOut);

        struct ExactInputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
        }

        function exactInput(
            ExactInputParams memory params
        ) external payable override checkDeadline(params.deadline) returns (uint256 amountOut);

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactOutputSingle(
            ExactOutputSingleParams calldata params
        ) external payable override checkDeadline(params.deadline) returns (uint256 amountIn);

        struct ExactOutputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
        }

        function exactOutput(
            ExactOutputParams calldata params
        ) external payable override checkDeadline(params.deadline) returns (uint256 amountIn);
    }

    #[derive(Debug)]
    #[sol(rpc)]
    contract IQuoterV2 {
        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }
        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
        public override returns (
            uint256 amountOut,
            uint160 sqrtPriceX96After,
            uint32 initializedTicksCrossed,
            uint256 gasEstimate
        );

        struct QuoteExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amount;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }
        function quoteExactOutputSingle(QuoteExactOutputSingleParams calldata params)
        public override returns (
            uint256 amountIn,
            uint160 sqrtPriceX96After,
            uint32 initializedTicksCrossed,
            uint256 gasEstimate
        );
    }

    contract UniversalRouter {
        struct V2SwapExactIn {
            address recipient;
            uint256 amountIn;
            uint256 amountOutMinimum;
            address[] path;
            bool payerIsUser;
            uint256[] minHopPriceX36;
        }

        struct V2SwapExactOut {
            address recipient;
            uint256 amountOut;
            uint256 amountInMax;
            address[] path;
            bool payerIsUser;
            uint256[] minHopPriceX36;
        }

        struct V3SwapExactIn {
            address recipient;
            uint256 amountIn;
            uint256 amountOutMinimum;
            bytes path;
            bool payerIsUser;
            uint256[] minHopPriceX36;
        }

        struct V3SwapExactOut {
            address recipient;
            uint256 amountOut;
            uint256 amountInMax;
            bytes path;
            bool payerIsUser;
            uint256[] minHopPriceX36;
        }

        function execute(bytes calldata commands, bytes[] calldata inputs, uint256 deadline)
            external payable checkDeadline(deadline);

        function execute(bytes calldata commands, bytes[] calldata inputs)
            public payable override isNotLocked;
    }

}
