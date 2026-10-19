// Trusted adapter only. User patterns and strings arrive as typed arguments, never source code.
globalThis.basesRegExp = (() => {
    const cache = new Map();
    function regexp(pattern, flags) {
        const key = pattern + "\0" + flags;
        let regex = cache.get(key);
        if (regex) cache.delete(key);
        else regex = new RegExp(pattern, flags);
        cache.set(key, regex);
        if (cache.size > 16) cache.delete(cache.keys().next().value);
        regex.lastIndex = 0;
        return regex;
    }
    function string(units) {
        let value = "";
        for (let i = 0; i < units.length; i += 1024) {
            value += String.fromCharCode.apply(null, units.slice(i, i + 1024));
        }
        return value;
    }
    function units(value) {
        if (value.length > 1048576) throw new RangeError("string result exceeds 1 Mi units");
        const result = [];
        for (let i = 0; i < value.length; i++) result.push(value.charCodeAt(i));
        return result;
    }
    return Object.freeze({
        matches(pattern, flags, input) {
            return regexp(pattern, flags).test(string(input));
        },
        replace(pattern, flags, isRegex, input, replacement) {
            const text = string(input), substitute = string(replacement);
            return units(isRegex
                ? text.replace(regexp(pattern, flags), substitute)
                : text.replaceAll(string(pattern), substitute));
        },
        split(pattern, flags, isRegex, input, limit) {
            const separator = isRegex ? regexp(pattern, flags) : string(pattern);
            return string(input).split(separator, limit).map(value =>
                value === undefined ? null : units(value));
        }
    });
})();
