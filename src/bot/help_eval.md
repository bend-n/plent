## how to use `}eval` <:micro_processor:1165059281087889479>

type```
}eval ​`​`​`arm
print "xd"
​`​`​`
```for the bot to evaluate your MLOG

you will have access to one large display.
you are capped to a maximum of `52789849` instructions.
you can set the number of iterations, by passing `}eval iters=10 ..`
iterations are clamped `1..=50`.
syntax errors will be gracefully reported, unknown instructions, such as `ubind`, `getlinks`, will be ignored.
labels are supported.
you may edit your message, and the mlog will be re-executed.
@variables, such as `@time`, `@tick`, are not supported yet.
