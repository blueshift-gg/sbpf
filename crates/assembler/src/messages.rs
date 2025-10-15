/* A central place to store all the label messages to avoid duplication
 * and make it easier to update or translate. We could move error messages here too
 */

pub const LABEL_REDEFINED: &str = "Label is redefined";

// we can do a more fine-grained error message by pointing out the exact token that caused the error
// with a special error pattern parser, but for now we just provide expected patterns
pub const EXPECTS_LABEL_DIR_STR: &str = "expects <label>, <directive>, <string literal>";
pub const EXPECTS_IDEN: &str = "expects <identifier>";
pub const EXPECTS_IDEN_COM_IMM: &str = "expects <identifier>, <immediate value>";
pub const EXPECTS_MORE_OPERAND: &str = "expects more operand";
pub const EXPECTS_REG: &str = "expects <register>";
pub const EXPECTS_REG_COM_IMM: &str = "expects <register>, <immediate value>";
pub const EXPECTS_REG_COM_REG: &str = "expects <register>, <register>";
pub const EXPECTS_REG_COM_IMM_OR_IDEN: &str = "expects <register>, <immediate value>/<identifier>";
pub const EXPECTS_REG_COM_IMM_COM_IMM_OR_IDEN: &str =
    "expects <register>, <immediate value>, <immediate value>/<identifier>";
pub const EXPECTS_REG_COM_LB_REG_BIOP_IMM_RB: &str =
    "expects <register>, [<register> <binary operator> <immediate value>]";
pub const EXPECTS_LB_REG_BIOP_IMM_RB_COM_REG: &str =
    "expects [<register> <binary operator> <immediate value>], <register>";
pub const EXPECTS_LB_REG_BIOP_IMM_RB_COM_IMM: &str =
    "expects [<register> <binary operator> <immediate value>], <immediate value>";
