//! 首页
use salvo::prelude::*;

#[handler]
pub async fn index(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Text::Plain("欢迎使用U验证V3"));
}
