import { Logger } from "tslog";
export default new Logger({
  prettyLogTemplate:
    "{{yyyy}}-{{mm}}-{{dd}}T{{hh}}:{{MM}}:{{ss}}:{{ms}}Z {{logLevelName}} [{{filePathWithLine}}{{name}}]\t",
});
