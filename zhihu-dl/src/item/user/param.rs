pub const ANSWER_INCLUDE:&str = "data[*].is_normal,admin_closed_comment,reward_info,is_collapsed,annotation_action,annotation_detail,collapse_reason,collapsed_by,suggest_edit,comment_count,can_comment,content,editable_content,attachment,voteup_count,reshipment_settings,comment_permission,mark_infos,created_time,updated_time,review_info,excerpt,is_labeled,label_info,relationship.is_authorized,voting,is_author,is_thanked,is_nothelp,is_recognized;data[*].vessay_info;data[*].author.badge[?(type=best_answerer)].topics;data[*].author.vip_info;data[*].question.has_publishing_draft,relationship";

pub const ARTICLE_INCLUDE:&str = "data[*].comment_count,suggest_edit,is_normal,thumbnail_extra_info,thumbnail,can_comment,comment_permission,admin_closed_comment,content,voteup_count,created,updated,upvoted_followees,voting,review_info,is_labeled,label_info;data[*].vessay_info;data[*].author.badge[?(type=best_answerer)].topics;data[*].author.vip_info;";

pub const COLUMN_INCLUDE: &str =
    "data[*].column.intro,followers,articles_count,voteup_count,items_count,description,created";

pub const CREATED_COLL_INCLUDE:&str = "data[*].updated_time,answer_count,follower_count,creator,description,is_following,comment_count,created_time;data[*].creator.vip_info";

pub const LIKED_COLL_INCLUDE:&str = "data[*].updated_time,answer_count,follower_count,creator,description,is_following,comment_count,created_time";
